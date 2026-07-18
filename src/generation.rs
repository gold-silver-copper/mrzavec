use crate::{
    map::{MAP_HEIGHT, MAP_WIDTH, Map, Pos, Terrain, Trap},
    rng::GameRng,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Room {
    pub id: u8,
    pub top_left: Pos,
    pub width: i32,
    pub height: i32,
    pub dark: bool,
    pub gone: bool,
    pub maze: bool,
    #[serde(default)]
    pub exits: Vec<Pos>,
    #[serde(default)]
    pub gold: Option<Pos>,
    #[serde(default)]
    pub gold_value: u32,
}
impl Room {
    pub fn center(&self) -> Pos {
        Pos::new(
            self.top_left.x + self.width / 2,
            self.top_left.y + self.height / 2,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    pub map: Map,
    pub rooms: Vec<Room>,
    #[serde(default)]
    pub passage_exits: Vec<Vec<Pos>>,
    pub stairs: Pos,
}

pub fn generate_level(rng: &mut GameRng, depth: u32) -> Level {
    let mut level = generate_layout(rng, depth);
    finish_level(rng, depth, &mut level, &[]);
    level
}

pub(crate) fn generate_layout(rng: &mut GameRng, depth: u32) -> Level {
    let mut level = begin_layout(rng);
    for id in 0..9u8 {
        dig_room(&mut level, id, depth, rng);
    }
    dig_passages(&mut level, depth, rng);
    level
}

pub(crate) fn begin_layout(rng: &mut GameRng) -> Level {
    let mut gone = [false; 9];
    let left_out = rng.rnd(4);
    for _ in 0..left_out {
        loop {
            let room = rng.rnd(9) as usize;
            if !gone[room] {
                gone[room] = true;
                break;
            }
        }
    }
    let rooms = (0..9u8)
        .map(|id| Room {
            id,
            top_left: Pos::new(0, 0),
            width: 0,
            height: 0,
            dark: false,
            gone: gone[id as usize],
            maze: false,
            exits: Vec::new(),
            gold: None,
            gold_value: 0,
        })
        .collect();
    Level {
        map: Map::default(),
        rooms,
        passage_exits: Vec::new(),
        stairs: Pos::new(0, 1),
    }
}

pub(crate) fn dig_room(level: &mut Level, id: u8, depth: u32, rng: &mut GameRng) {
    let cell_w = MAP_WIDTH / 3;
    // Rogue partitions the full 24-line terminal, including the status line,
    // into three eight-line room bands. Playable room coordinates still end
    // at row 22.
    let cell_h = (MAP_HEIGHT + 1) / 3;
    let col = id as i32 % 3;
    let row = id as i32 / 3;
    let gone = level.rooms[id as usize].gone;
    let dark_candidate = !gone && rng.rnd(10) < depth.saturating_sub(1);
    let maze = dark_candidate && rng.rnd(15) == 0;
    let dark = dark_candidate && !maze;
    let (width, height, x, y) = if gone {
        loop {
            let x = col * cell_w + 2 + rng.rnd((cell_w - 2) as u32) as i32;
            let y = row * cell_h + rng.rnd((cell_h - 2) as u32) as i32 + 1;
            if y > 0 && y < MAP_HEIGHT - 1 {
                break (-MAP_WIDTH, -(MAP_HEIGHT + 1), x, y);
            }
        }
    } else if maze {
        let width = cell_w - 1;
        let mut height = cell_h - 1;
        let top_x = col * cell_w + 1;
        let x = if top_x == 1 { 0 } else { top_x };
        let mut y = row * cell_h;
        if y == 0 {
            y += 1;
            height -= 1;
        }
        (width, height, x, y)
    } else {
        loop {
            let width = 4 + rng.rnd((cell_w - 4) as u32) as i32;
            let mut height = 4 + rng.rnd((cell_h - 4) as u32) as i32;
            let x = col * cell_w + 1 + rng.rnd((cell_w - width).max(1) as u32) as i32;
            let mut y = row * cell_h + rng.rnd((cell_h - height).max(1) as u32) as i32;
            if id > 3
                && level.rooms[(id - 3) as usize].maze
                && level.rooms[(id - 3) as usize].top_left.y + level.rooms[(id - 3) as usize].height
                    == y - 1
            {
                y += 1;
                if height > 4 {
                    height -= 1;
                }
            }
            if y != 0 {
                break (width, height, x, y);
            }
        }
    };
    let room = Room {
        id,
        top_left: Pos::new(x, y),
        width,
        height,
        dark,
        gone,
        maze,
        exits: Vec::new(),
        gold: None,
        gold_value: 0,
    };
    if maze {
        carve_maze(&mut level.map, &room, depth, rng)
    } else if !gone {
        carve_room(&mut level.map, &room);
    }
    level.rooms[id as usize] = room;
}

pub(crate) fn dig_passages(level: &mut Level, depth: u32, rng: &mut GameRng) {
    const ADJACENT: [[bool; 9]; 9] = [
        [false, true, false, true, false, false, false, false, false],
        [true, false, true, false, true, false, false, false, false],
        [false, true, false, false, false, true, false, false, false],
        [true, false, false, false, true, false, true, false, false],
        [false, true, false, true, false, true, false, true, false],
        [false, false, true, false, true, false, false, false, true],
        [false, false, false, true, false, false, false, true, false],
        [false, false, false, false, true, false, true, false, true],
        [false, false, false, false, false, true, false, true, false],
    ];
    let mut connected = [[false; 9]; 9];
    let mut in_graph = [false; 9];
    let mut current = rng.rnd(9) as usize;
    let mut provisional_passage = 0u8;
    in_graph[current] = true;
    let mut room_count = 1;
    while room_count < 9 {
        let mut candidate = None;
        let mut count = 0;
        for next in 0..9 {
            if ADJACENT[current][next] && !in_graph[next] {
                count += 1;
                if rng.rnd(count) == 0 {
                    candidate = Some(next);
                }
            }
        }
        if let Some(next) = candidate {
            carve_connection(
                &mut level.map,
                &mut level.rooms,
                current,
                next,
                provisional_passage,
                depth,
                rng,
            );
            provisional_passage = provisional_passage.wrapping_add(1);
            connected[current][next] = true;
            connected[next][current] = true;
            in_graph[next] = true;
            room_count += 1;
        } else {
            loop {
                current = rng.rnd(9) as usize;
                if in_graph[current] {
                    break;
                }
            }
        }
    }
    for _ in 0..rng.rnd(5) {
        let from = rng.rnd(9) as usize;
        let mut candidate = None;
        let mut count = 0;
        for to in 0..9 {
            if ADJACENT[from][to] && !connected[from][to] {
                count += 1;
                if rng.rnd(count) == 0 {
                    candidate = Some(to);
                }
            }
        }
        if let Some(to) = candidate {
            carve_connection(
                &mut level.map,
                &mut level.rooms,
                from,
                to,
                provisional_passage,
                depth,
                rng,
            );
            provisional_passage = provisional_passage.wrapping_add(1);
            connected[from][to] = true;
            connected[to][from] = true;
        }
    }
    level.passage_exits = number_passage_components(&mut level.map, &level.rooms);
}

pub(crate) fn finish_level(
    rng: &mut GameRng,
    depth: u32,
    level: &mut Level,
    occupied_by_items: &[Pos],
) {
    if rng.rnd(10) < depth {
        let count = (rng.rnd(depth / 4) + 1).min(10);
        for _ in 0..count {
            let p = loop {
                let Some(p) = find_floor_position(&level.map, &level.rooms, rng, occupied_by_items)
                else {
                    break None;
                };
                if level
                    .map
                    .get(p)
                    .is_some_and(|cell| cell.terrain == Terrain::Floor)
                {
                    break Some(p);
                }
            };
            if let Some(p) = p {
                let c = level.map.get_mut(p).expect("trap floor");
                c.trap = Some(match rng.rnd(8) {
                    0 => Trap::TrapDoor,
                    1 => Trap::Arrow,
                    2 => Trap::SleepGas,
                    3 => Trap::Bear,
                    4 => Trap::Teleport,
                    5 => Trap::PoisonDart,
                    6 => Trap::Rust,
                    _ => Trap::Mysterious,
                });
            }
        }
    }
    let stairs = find_floor_position(&level.map, &level.rooms, rng, occupied_by_items)
        .expect("generated floor for stairs");
    let stair_cell = level.map.get_mut(stairs).expect("stairs in bounds");
    stair_cell.terrain = Terrain::Stairs;
    stair_cell.trap = None;
    level.stairs = stairs;
}

/// Mirror `passnum`/`numpass`: passage numbers identify whole orthogonally
/// connected corridor networks, including their doors and maze-room passages.
fn number_passage_components(map: &mut Map, rooms: &[Room]) -> Vec<Vec<Pos>> {
    fn numerable(terrain: Terrain) -> bool {
        matches!(
            terrain,
            Terrain::Passage
                | Terrain::SecretPassage
                | Terrain::Door
                | Terrain::SecretDoor
                | Terrain::SecretDoorHorizontal
                | Terrain::SecretDoorVertical
        )
    }

    fn number_from(map: &mut Map, pos: Pos, passage: u8, exits: &mut Vec<Pos>) {
        if pos.x < 0 || pos.x >= MAP_WIDTH || pos.y <= 0 || pos.y >= MAP_HEIGHT {
            return;
        }
        let Some(cell) = map.get(pos) else {
            return;
        };
        if cell.passage.is_some() || !numerable(cell.terrain) {
            return;
        }
        if matches!(
            cell.terrain,
            Terrain::Door
                | Terrain::SecretDoor
                | Terrain::SecretDoorHorizontal
                | Terrain::SecretDoorVertical
        ) {
            exits.push(pos);
        }
        map.get_mut(pos).expect("numbered passage cell").passage = Some(passage);
        for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
            number_from(map, pos.offset(dx, dy), passage, exits);
        }
    }

    for y in 1..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            map.get_mut(Pos::new(x, y)).expect("map coordinate").passage = None;
        }
    }
    let mut passage = 0u8;
    let mut passage_exits = vec![Vec::new()];
    for room in rooms {
        for &exit in &room.exits {
            if map.get(exit).is_some_and(|cell| cell.passage.is_none()) {
                passage = passage.checked_add(1).expect("passage count fits u8");
                passage_exits.push(Vec::new());
                number_from(map, exit, passage, &mut passage_exits[passage as usize]);
            }
        }
    }
    passage_exits
}

fn find_floor_position(
    map: &Map,
    rooms: &[Room],
    rng: &mut GameRng,
    occupied_by_items: &[Pos],
) -> Option<Pos> {
    for _ in 0..100_000 {
        let room = &rooms[rng.rnd(rooms.len() as u32) as usize];
        if room.gone {
            continue;
        }
        let pos = Pos::new(
            room.top_left.x + rng.rnd((room.width - 2) as u32) as i32 + 1,
            room.top_left.y + rng.rnd((room.height - 2) as u32) as i32 + 1,
        );
        let expected = if room.maze {
            Terrain::Passage
        } else {
            Terrain::Floor
        };
        if !occupied_by_items.contains(&pos)
            && map.get(pos).is_some_and(|cell| cell.terrain == expected)
        {
            return Some(pos);
        }
    }
    None
}

fn carve_maze(map: &mut Map, room: &Room, depth: u32, rng: &mut GameRng) {
    let min_x = room.top_left.x;
    let min_y = room.top_left.y;
    // `do_maze` treats r_max as an inclusive coordinate bound, unlike normal
    // room drawing's size semantics.
    let max_x = (min_x + room.width).min(MAP_WIDTH - 1);
    let max_y = (min_y + room.height).min(MAP_HEIGHT - 1);
    let start_y = min_y + (rng.rnd(room.height as u32) as i32 / 2) * 2;
    let start_x = min_x + (rng.rnd(room.width as u32) as i32 / 2) * 2;
    let start = Pos::new(start_x, start_y);
    let mut stack = vec![start];
    put_passage(map, start, 0, depth, rng);
    if let Some(c) = map.get_mut(start) {
        c.room = Some(room.id)
    }
    while let Some(&at) = stack.last() {
        let mut choice = None;
        let mut count = 0;
        for (dx, dy) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
            let next = at.offset(dx, dy);
            if next.x >= min_x
                && next.x <= max_x
                && next.y >= min_y
                && next.y <= max_y
                && map.get(next).is_some_and(|c| c.terrain == Terrain::Void)
            {
                count += 1;
                if rng.rnd(count) == 0 {
                    choice = Some((next, dx / 2, dy / 2));
                }
            }
        }
        let Some((next, dx, dy)) = choice else {
            stack.pop();
            continue;
        };
        for p in [at.offset(dx, dy), next] {
            put_passage(map, p, 0, depth, rng);
            if let Some(c) = map.get_mut(p) {
                c.room = Some(room.id)
            }
        }
        stack.push(next)
    }
}

fn carve_room(map: &mut Map, r: &Room) {
    for y in r.top_left.y..r.top_left.y + r.height {
        for x in r.top_left.x..r.top_left.x + r.width {
            let border_y = y == r.top_left.y || y == r.top_left.y + r.height - 1;
            let border_x = x == r.top_left.x || x == r.top_left.x + r.width - 1;
            let terrain = if border_y {
                Terrain::WallHorizontal
            } else if border_x {
                Terrain::WallVertical
            } else {
                Terrain::Floor
            };
            if let Some(c) = map.get_mut(Pos::new(x, y)) {
                c.terrain = terrain;
                c.room = Some(r.id)
            }
        }
    }
}

fn carve_connection(
    map: &mut Map,
    rooms: &mut [Room],
    a: usize,
    b: usize,
    passage: u8,
    depth: u32,
    rng: &mut GameRng,
) {
    let a = rooms[a].clone();
    let b = rooms[b].clone();
    let (from, to) = if a.id < b.id { (&a, &b) } else { (&b, &a) };
    let horizontal = to.id == from.id + 1;
    let (start, end, delta, turn_delta) = if horizontal {
        let start = side_point(map, from, true, true, rng);
        let end = side_point(map, to, true, false, rng);
        (start, end, (1, 0), (0, (end.y - start.y).signum()))
    } else {
        let start = side_point(map, from, false, true, rng);
        let end = side_point(map, to, false, false, rng);
        (start, end, (0, 1), ((end.x - start.x).signum(), 0))
    };
    let mut distance = if horizontal {
        (start.x - end.x).abs() - 1
    } else {
        (start.y - end.y).abs() - 1
    };
    let mut turn_distance = if horizontal {
        (start.y - end.y).abs()
    } else {
        (start.x - end.x).abs()
    };
    let turn_spot = rng.rnd((distance - 1).max(0) as u32) as i32 + 1;
    if !from.gone {
        rooms[from.id as usize].exits.push(start);
    }
    if !to.gone {
        rooms[to.id as usize].exits.push(end);
    }
    open_endpoint(map, start, from, passage, depth, rng);
    open_endpoint(map, end, to, passage, depth, rng);
    let mut current = start;
    while distance > 0 {
        current = current.offset(delta.0, delta.1);
        if distance == turn_spot {
            while turn_distance > 0 {
                put_passage(map, current, passage, depth, rng);
                current = current.offset(turn_delta.0, turn_delta.1);
                turn_distance -= 1;
            }
        }
        put_passage(map, current, passage, depth, rng);
        distance -= 1;
    }
    // The C routine merely prints "connectivity problem" when adjacent room
    // walls leave no primary segment on which to perform the lateral turn.
    // Preserve its route otherwise, but bridge that demonstrably disconnected
    // final gap (documented in BUG_FIXES.md).
    while (current.x - end.x).abs() + (current.y - end.y).abs() > 1 {
        let (dx, dy) = if current.x != end.x {
            ((end.x - current.x).signum(), 0)
        } else {
            (0, (end.y - current.y).signum())
        };
        current = current.offset(dx, dy);
        put_passage(map, current, passage, depth, rng);
    }
}

fn side_point(map: &Map, room: &Room, horizontal: bool, far_side: bool, rng: &mut GameRng) -> Pos {
    if room.gone {
        return room.top_left;
    }
    let candidates: Vec<Pos> = if horizontal {
        let x = room.top_left.x + if far_side { room.width - 1 } else { 0 };
        (room.top_left.y + 1..room.top_left.y + room.height - 1)
            .map(|y| Pos::new(x, y))
            .collect()
    } else {
        let y = room.top_left.y + if far_side { room.height - 1 } else { 0 };
        (room.top_left.x + 1..room.top_left.x + room.width - 1)
            .map(|x| Pos::new(x, y))
            .collect()
    };
    for _ in 0..10_000 {
        let candidate = candidates[rng.rnd(candidates.len() as u32) as usize];
        if !room.maze
            || map.get(candidate).is_some_and(|cell| {
                matches!(cell.terrain, Terrain::Passage | Terrain::SecretPassage)
            })
        {
            return candidate;
        }
    }
    candidates[0]
}

fn open_endpoint(map: &mut Map, pos: Pos, room: &Room, passage: u8, depth: u32, rng: &mut GameRng) {
    if room.gone {
        put_passage(map, pos, passage, depth, rng);
    } else if !room.maze {
        let secret = rng.rnd(10) + 1 < depth && rng.rnd(5) == 0;
        let cell = map.get_mut(pos).expect("room endpoint in bounds");
        let already_secret = matches!(
            cell.terrain,
            Terrain::SecretDoor | Terrain::SecretDoorHorizontal | Terrain::SecretDoorVertical
        );
        cell.terrain = if secret || already_secret {
            if pos.y == room.top_left.y || pos.y == room.top_left.y + room.height - 1 {
                Terrain::SecretDoorHorizontal
            } else {
                Terrain::SecretDoorVertical
            }
        } else {
            Terrain::Door
        };
        cell.room = Some(room.id);
        cell.passage = Some(passage);
    }
}

fn put_passage(map: &mut Map, pos: Pos, passage: u8, depth: u32, rng: &mut GameRng) {
    let secret = rng.rnd(10) + 1 < depth && rng.rnd(40) == 0;
    if let Some(cell) = map.get_mut(pos) {
        cell.terrain = if secret || cell.terrain == Terrain::SecretPassage {
            Terrain::SecretPassage
        } else {
            Terrain::Passage
        };
        cell.passage = Some(passage);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};
    #[test]
    fn generated_levels_are_connected() {
        for seed in 1..100 {
            let l = generate_level(&mut GameRng::new(seed), 1);
            let start = l.rooms.iter().find(|r| !r.gone).unwrap().center();
            let mut seen = HashSet::new();
            let mut q = VecDeque::from([start]);
            while let Some(p) = q.pop_front() {
                if !seen.insert(p) {
                    continue;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = p.offset(dx, dy);
                    if l.map.get(n).is_some_and(|c| {
                        c.terrain.passable()
                            || matches!(
                                c.terrain,
                                Terrain::SecretPassage
                                    | Terrain::SecretDoor
                                    | Terrain::SecretDoorHorizontal
                                    | Terrain::SecretDoorVertical
                            )
                    }) && !seen.contains(&n)
                    {
                        q.push_back(n)
                    }
                }
            }
            for r in &l.rooms {
                if !r.gone {
                    assert!(
                        seen.contains(&r.center()),
                        "seed {seed}, room {} {r:?}",
                        r.id
                    )
                }
            }
        }
    }

    #[test]
    fn passage_numbers_cover_connected_corridor_components() {
        for seed in 1..500 {
            let level = generate_level(&mut GameRng::new(seed), 20);
            for (pos, cell) in level.map.iter().filter(|(_, cell)| {
                matches!(
                    cell.terrain,
                    Terrain::Passage
                        | Terrain::SecretPassage
                        | Terrain::Door
                        | Terrain::SecretDoor
                        | Terrain::SecretDoorHorizontal
                        | Terrain::SecretDoorVertical
                )
            }) {
                let passage = cell.passage.expect("every corridor cell is numbered");
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let Some(neighbor) = level.map.get(pos.offset(dx, dy)) else {
                        continue;
                    };
                    if neighbor.passage.is_some() {
                        assert_eq!(neighbor.passage, Some(passage), "seed {seed} at {pos:?}");
                    }
                }
            }
        }
    }
    #[test]
    fn stairs_are_on_passable_floor() {
        let l = generate_level(&mut GameRng::new(9), 26);
        assert_eq!(l.map.get(l.stairs).unwrap().terrain, Terrain::Stairs)
    }
    #[test]
    fn missing_rooms_match_original_zero_to_three_range() {
        for seed in 1..1000 {
            let l = generate_level(&mut GameRng::new(seed), 20);
            assert!(l.rooms.iter().filter(|r| r.gone).count() <= 3)
        }
    }
    #[test]
    fn room_bands_use_the_original_eight_line_partition() {
        let mut found_height_seven = false;
        for seed in 1..1000 {
            let level = generate_level(&mut GameRng::new(seed), 1);
            for room in level.rooms.iter().filter(|room| !room.gone && !room.maze) {
                found_height_seven |= room.height == 7;
                assert!((1..MAP_HEIGHT).contains(&room.top_left.y));
                assert!(room.top_left.y + room.height <= MAP_HEIGHT);
                assert_eq!(room.top_left.y / 8, i32::from(room.id) / 3);
            }
        }
        assert!(found_height_seven);
    }

    #[test]
    fn stairs_can_be_placed_in_maze_passages() {
        let mut found = false;
        for seed in 1..10_000 {
            let level = generate_level(&mut GameRng::new(seed), 20);
            let room = level.map.get(level.stairs).unwrap().room.unwrap();
            if level.rooms[room as usize].maze {
                found = true;
                break;
            }
        }
        assert!(found);
    }
    #[test]
    fn maze_rooms_contain_walkable_passages() {
        let mut found = 0;
        for seed in 1..5000 {
            let l = generate_level(&mut GameRng::new(seed), 20);
            for room in l.rooms.iter().filter(|r| r.maze) {
                found += 1;
                assert!(
                    l.map
                        .iter()
                        .any(|(_, c)| c.room == Some(room.id) && c.terrain == Terrain::Passage)
                )
            }
        }
        assert!(found > 0)
    }
    #[test]
    fn maze_rooms_replace_darkness_instead_of_combining_with_it() {
        for seed in 1..5000 {
            let level = generate_level(&mut GameRng::new(seed), 20);
            assert!(level.rooms.iter().all(|room| !room.maze || !room.dark));
        }
    }

    #[test]
    fn traps_are_never_placed_in_maze_rooms() {
        let mut saw_maze = false;
        let mut saw_trap = false;
        for seed in 1..5000 {
            let level = generate_level(&mut GameRng::new(seed), 30);
            saw_maze |= level.rooms.iter().any(|room| room.maze);
            for (_, cell) in level.map.iter().filter(|(_, cell)| cell.trap.is_some()) {
                saw_trap = true;
                assert!(
                    cell.room
                        .and_then(|room| level.rooms.get(room as usize))
                        .is_none_or(|room| !room.maze)
                );
            }
        }
        assert!(saw_maze && saw_trap);
    }

    #[test]
    fn deep_levels_can_contain_searchable_secret_passages() {
        let mut found = false;
        for seed in 1..5000 {
            let level = generate_level(&mut GameRng::new(seed), 30);
            if level
                .map
                .iter()
                .any(|(_, cell)| cell.terrain == Terrain::SecretPassage)
            {
                found = true;
                break;
            }
        }
        assert!(found);
    }

    #[test]
    fn overlapping_corridors_do_not_unhide_a_secret_passage() {
        let mut map = Map::default();
        let pos = Pos::new(20, 10);
        map.get_mut(pos).unwrap().terrain = Terrain::SecretPassage;

        put_passage(&mut map, pos, 1, 1, &mut GameRng::new(1));

        assert_eq!(map.get(pos).unwrap().terrain, Terrain::SecretPassage);
    }

    #[test]
    fn secret_doors_preserve_their_wall_orientation() {
        let mut found_horizontal = false;
        let mut found_vertical = false;
        for seed in 1..5000 {
            let level = generate_level(&mut GameRng::new(seed), 30);
            for (pos, cell) in level.map.iter() {
                let Some(room_id) = cell.room else { continue };
                let room = &level.rooms[room_id as usize];
                match cell.terrain {
                    Terrain::SecretDoorHorizontal => {
                        found_horizontal = true;
                        assert!(
                            pos.y == room.top_left.y || pos.y == room.top_left.y + room.height - 1
                        );
                        assert_eq!(cell.terrain.glyph(), '-');
                    }
                    Terrain::SecretDoorVertical => {
                        found_vertical = true;
                        assert!(
                            pos.x == room.top_left.x || pos.x == room.top_left.x + room.width - 1
                        );
                        assert_eq!(cell.terrain.glyph(), '|');
                    }
                    _ => {}
                }
            }
        }
        assert!(found_horizontal && found_vertical);
    }
}
