use crate::{
    Game,
    game::EndState,
    item::{ARMOR_CLASS, ItemKind},
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    Killed,
    Quit,
    Winner,
    KilledWithAmulet,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreEntry {
    pub score: u32,
    pub name: String,
    pub reason: Reason,
    #[serde(default)]
    pub cause: Option<String>,
    pub level: u32,
    pub when: u64,
}

const WEAPON_WORTH: [i32; 9] = [8, 15, 15, 1, 3, 75, 2, 5, 5];
const ARMOR_WORTH: [i32; 8] = [20, 25, 20, 30, 75, 80, 90, 150];
const POTION_WORTH: [i32; 14] = [5, 5, 5, 150, 100, 130, 130, 105, 250, 200, 190, 130, 5, 75];
const SCROLL_WORTH: [i32; 18] = [
    140, 150, 180, 5, 160, 80, 80, 80, 100, 115, 200, 60, 165, 150, 75, 105, 20, 250,
];
const RING_WORTH: [i32; 14] = [
    400, 400, 280, 420, 310, 10, 10, 440, 400, 460, 240, 30, 470, 380,
];
const STICK_WORTH: [i32; 14] = [
    250, 5, 330, 330, 330, 310, 170, 5, 350, 300, 5, 340, 50, 280,
];

pub fn amount(game: &Game) -> u32 {
    let amount = match game.end {
        EndState::Dead if game.death_cause.as_deref() == Some("signal") => {
            i64::from(game.player.gold)
        }
        EndState::Dead => i64::from(game.player.gold - game.player.gold / 10),
        EndState::Quit | EndState::Playing => i64::from(game.player.gold),
        EndState::Won => i64::from(game.player.gold) + i64::from(loot_worth(game)),
    };
    amount.clamp(0, i64::from(u32::MAX)) as u32
}
pub fn loot_worth(game: &Game) -> u32 {
    let mut previous = 0;
    game.player
        .inventory
        .iter()
        .filter(|item| item.in_pack)
        .map(|item| {
            previous = item_worth_after(game, item, previous);
            previous
        })
        .sum()
}
pub fn item_worth(game: &Game, item: &crate::item::Item) -> u32 {
    item_worth_after(game, item, 0)
}

/// `total_winner` leaves its loop-local `worth` unchanged for master-created
/// gold and arbitrary object glyphs because neither has a switch arm.
pub fn item_worth_after(game: &Game, item: &crate::item::Item, previous: u32) -> u32 {
    let count = item.count as i32;
    let worth = match item.kind {
        ItemKind::Food => 2 * count,
        ItemKind::Weapon => {
            WEAPON_WORTH[item.which as usize]
                * (3 * (item.hit_plus as i32 + item.damage_plus as i32) + count)
        }
        ItemKind::Armor => {
            let armor = item.armor_class.unwrap_or(10);
            ARMOR_WORTH[item.which as usize]
                + (9 - armor) * 100
                + 10 * (ARMOR_CLASS[item.which as usize] - armor)
        }
        ItemKind::Scroll => {
            SCROLL_WORTH[item.which as usize] * count
                / if game.knowledge.scrolls[item.which as usize] {
                    1
                } else {
                    2
                }
        }
        ItemKind::Potion => {
            POTION_WORTH[item.which as usize] * count
                / if game.knowledge.potions[item.which as usize] {
                    1
                } else {
                    2
                }
        }
        ItemKind::Ring => {
            let mut v = RING_WORTH[item.which as usize]
                + game.appearances.ring_stone_values[item.which as usize];
            if matches!(item.which, 0 | 1 | 7 | 8) {
                let bonus = item.armor_class.unwrap_or(0);
                v = if bonus > 0 { v + bonus * 100 } else { 10 }
            }
            if item.known { v } else { v / 2 }
        }
        ItemKind::Stick => {
            let v = STICK_WORTH[item.which as usize] + 20 * item.charges as i32;
            if item.known { v } else { v / 2 }
        }
        ItemKind::Amulet => 1000,
        ItemKind::Gold | ItemKind::Bizarre(_) => return previous,
    };
    worth.max(0) as u32
}

pub fn record(game: &Game, path: &Path) -> io::Result<Vec<ScoreEntry>> {
    if game.no_score || game.end == EndState::Playing {
        return existing(path);
    }
    let (scores, changed) = ranked(game, path)?;
    if changed {
        write(path, &scores)?;
    }
    Ok(scores)
}
fn ranked(game: &Game, path: &Path) -> io::Result<(Vec<ScoreEntry>, bool)> {
    let mut scores = existing(path)?;
    if game.no_score {
        return Ok((scores, false));
    }
    let reason = match game.end {
        EndState::Dead if game.has_amulet => Reason::KilledWithAmulet,
        EndState::Dead => Reason::Killed,
        EndState::Quit => Reason::Quit,
        EndState::Won => Reason::Winner,
        EndState::Playing => return Ok((scores, false)),
    };
    let name = game.options.name.clone();
    let when = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let new_score = amount(game);
    if new_score == 0 {
        return Ok((scores, false));
    }
    if reason != Reason::Winner {
        if scores.iter().any(|entry| {
            entry.name == name && entry.reason != Reason::Winner && entry.score >= new_score
        }) {
            return Ok((scores, false));
        }
        scores.retain(|entry| entry.name != name || entry.reason == Reason::Winner);
    }
    scores.push(ScoreEntry {
        score: new_score,
        name,
        reason,
        cause: game.death_cause.clone(),
        level: if reason == Reason::Winner {
            game.max_depth
        } else {
            game.depth
        },
        when,
    });
    scores.sort_by_key(|s| std::cmp::Reverse(s.score));
    scores.truncate(10);
    Ok((scores, true))
}
fn existing(path: &Path) -> io::Result<Vec<ScoreEntry>> {
    match read(path) {
        Ok(scores) => Ok(scores),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}
fn write(path: &Path, scores: &[ScoreEntry]) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(&scores).map_err(io::Error::other)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)
}
pub fn record_locked(game: &Game, path: &Path, lock_path: &Path) -> io::Result<Vec<ScoreEntry>> {
    if game.no_score || game.end == EndState::Playing {
        return existing(path);
    }
    let (scores, changed) = ranked(game, path)?;
    if !changed {
        return Ok(scores);
    }
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let Ok(_lock) = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    else {
        return Ok(scores);
    };
    struct RemoveLock<'a>(&'a Path);
    impl Drop for RemoveLock<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.0);
        }
    }
    let _remove_lock = RemoveLock(lock_path);
    write(path, &scores)?;
    Ok(scores)
}
pub fn read(path: &Path) -> io::Result<Vec<ScoreEntry>> {
    serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
}
pub fn format(scores: &[ScoreEntry]) -> String {
    let mut out = String::from("Top 10 Rogueists:\n   Score Name\n");
    for (i, s) in scores.iter().enumerate() {
        out.push_str(&format!(
            "{:2} {:5} {}: {} on level {}",
            i + 1,
            s.score,
            s.name,
            reason_text(s.reason),
            s.level
        ));
        if matches!(s.reason, Reason::Killed | Reason::KilledWithAmulet)
            && let Some(cause) = &s.cause
        {
            out.push_str(&format!(" by {cause}"));
        }
        out.push_str(".\n")
    }
    out
}
fn reason_text(reason: Reason) -> &'static str {
    match reason {
        Reason::Killed => "killed",
        Reason::Quit => "quit",
        Reason::Winner => "A total winner",
        Reason::KilledWithAmulet => "killed with Amulet",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn death_takes_ten_percent() {
        let mut g = Game::new(1);
        g.player.gold = 101;
        g.end = EndState::Dead;
        assert_eq!(amount(&g), 91)
    }

    #[test]
    fn negative_wizard_gold_never_wraps_into_a_score() {
        let mut game = Game::new(1010);
        game.player.gold = -50;
        game.end = EndState::Quit;

        assert_eq!(amount(&game), 0);
    }
    #[test]
    fn winner_sells_amulet() {
        let mut g = Game::new(2);
        let mut a = crate::item::Item::basic(999, ItemKind::Amulet, 0);
        a.pos = None;
        g.player.inventory.push(a);
        g.end = EndState::Won;
        assert!(amount(&g) >= 1000)
    }

    #[test]
    fn master_only_object_types_reuse_the_previous_winner_sale_worth() {
        let mut game = Game::new(20);
        game.player.inventory.clear();
        let mut food = crate::item::Item::basic(1, ItemKind::Food, 0);
        food.count = 3;
        let gold = crate::item::Item::gold(2, 50_000);
        let bizarre = crate::item::Item::basic(3, ItemKind::Bizarre('x'), 0);
        game.player.inventory.extend([food, gold, bizarre]);

        assert_eq!(item_worth_after(&game, &game.player.inventory[0], 0), 6);
        assert_eq!(item_worth_after(&game, &game.player.inventory[1], 6), 6);
        assert_eq!(item_worth_after(&game, &game.player.inventory[2], 6), 6);
        assert_eq!(loot_worth(&game), 18);
    }

    #[test]
    fn ring_sale_uses_random_stone_value_and_item_identification() {
        let mut g = Game::new(3);
        let before = loot_worth(&g);
        let mut ring = crate::item::Item::basic(999, ItemKind::Ring, 0);
        ring.armor_class = Some(1);
        g.player.inventory.push(ring);
        let full = (RING_WORTH[0] + g.appearances.ring_stone_values[0] + 100) as u32;
        assert_eq!(loot_worth(&g) - before, full / 2);
        g.knowledge.rings[0] = true;
        assert_eq!(loot_worth(&g) - before, full / 2);
        g.player.inventory.last_mut().unwrap().known = true;
        assert_eq!(loot_worth(&g) - before, full);
    }

    #[test]
    fn stick_sale_uses_item_identification_not_global_discovery() {
        let mut g = Game::new(4);
        let before = loot_worth(&g);
        let mut stick = crate::item::Item::basic(999, ItemKind::Stick, 0);
        stick.charges = 5;
        stick.known = false;
        g.player.inventory.push(stick);
        let full = (STICK_WORTH[0] + 100) as u32;
        assert_eq!(loot_worth(&g) - before, full / 2);
        g.knowledge.sticks[0] = true;
        assert_eq!(loot_worth(&g) - before, full / 2);
        g.player.inventory.last_mut().unwrap().known = true;
        assert_eq!(loot_worth(&g) - before, full);
    }

    #[test]
    fn death_score_names_the_cause() {
        let scores = [ScoreEntry {
            score: 123,
            name: "rogue".into(),
            reason: Reason::KilledWithAmulet,
            cause: Some("a dragon".into()),
            level: 26,
            when: 0,
        }];
        assert!(format(&scores).contains("killed with Amulet on level 26 by a dragon."));
    }

    #[test]
    fn record_keeps_only_the_best_nonwinner_for_a_player() {
        let p =
            std::env::temp_dir().join(format!("mrzavec-score-best-{}.json", std::process::id()));
        let mut game = Game::new(11);
        game.options.name = "same player".into();
        game.end = EndState::Quit;
        game.player.gold = 100;
        record(&game, &p).unwrap();
        game.player.gold = 90;
        assert_eq!(record(&game, &p).unwrap().len(), 1);
        game.player.gold = 110;
        let scores = record(&game, &p).unwrap();
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].score, 110);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn corrupt_score_file_is_not_overwritten() {
        let p = std::env::temp_dir().join(format!("mrzavec-score-bad-{}.json", std::process::id()));
        fs::write(&p, b"broken").unwrap();
        let mut game = Game::new(12);
        game.end = EndState::Quit;
        assert!(record(&game, &p).is_err());
        assert_eq!(fs::read(&p).unwrap(), b"broken");
        let _ = fs::remove_file(p);
    }

    #[test]
    fn configured_lock_prevents_concurrent_score_update() {
        let base = std::env::temp_dir().join(format!("mrzavec-score-lock-{}", std::process::id()));
        let scores = base.with_extension("scores");
        let lock = base.with_extension("lock");
        fs::write(&lock, b"held").unwrap();
        let mut game = Game::new(13);
        game.end = EndState::Quit;
        game.player.gold = 123;
        let displayed = record_locked(&game, &scores, &lock).unwrap();
        assert_eq!(displayed[0].score, 123);
        assert!(!scores.exists());
        let _ = fs::remove_file(lock);
    }

    #[test]
    fn unscored_games_neither_create_nor_update_the_table() {
        let p = std::env::temp_dir().join(format!("mrzavec-no-score-{}.json", std::process::id()));
        let mut game = Game::new(14);
        game.end = EndState::Quit;
        game.no_score = true;
        assert!(record(&game, &p).unwrap().is_empty());
        assert!(!p.exists());
    }

    #[test]
    fn zero_point_games_do_not_enter_or_create_the_score_table() {
        let p = std::env::temp_dir().join(format!(
            "mrzavec-zero-score-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut game = Game::new(15);
        game.options.name = "penniless".into();
        game.end = EndState::Quit;
        game.player.gold = 0;

        assert!(record(&game, &p).unwrap().is_empty());
        assert!(!p.exists());
    }

    #[test]
    fn zero_point_game_does_not_displace_an_existing_score() {
        let p = std::env::temp_dir().join(format!(
            "mrzavec-zero-score-existing-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let existing = vec![ScoreEntry {
            score: 1,
            name: "survivor".into(),
            reason: Reason::Quit,
            cause: None,
            level: 1,
            when: 0,
        }];
        write(&p, &existing).unwrap();
        let mut game = Game::new(16);
        game.end = EndState::Dead;

        assert_eq!(record(&game, &p).unwrap(), existing);
        assert_eq!(read(&p).unwrap(), existing);
        let _ = fs::remove_file(p);
    }
}
