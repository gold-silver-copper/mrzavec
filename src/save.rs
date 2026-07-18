use crate::Game;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn save(game: &Game, path: &Path) -> io::Result<()> {
    let bytes = serde_json::to_vec(game).map_err(io::Error::other)?;
    let (tmp, mut file) = create_temporary(path)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        set_save_permissions(&file)?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
fn create_temporary(path: &Path) -> io::Result<(PathBuf, File)> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("save");
    for _ in 0..100 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".{name}.tmp-{}-{sequence}", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a unique temporary save file",
    ))
}
fn set_save_permissions(file: &File) -> io::Result<()> {
    let mut permissions = file.metadata()?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o400);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(true);
    file.set_permissions(permissions)
}
pub fn load(path: &Path) -> io::Result<Game> {
    let bytes = fs::read(path)?;
    decode(&bytes)
}
fn decode(bytes: &[u8]) -> io::Result<Game> {
    let game: Game = serde_json::from_slice(bytes).map_err(io::Error::other)?;
    if game.save_version != 12 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported save version",
        ));
    }
    if game.end != crate::game::EndState::Playing || game.player.stats.hp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot restore a finished or dead game",
        ));
    }
    Ok(game)
}
pub fn restore(path: &Path) -> io::Result<Game> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "save file must not be a symbolic link",
        ));
    }
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "save file must be a regular file",
        ));
    }
    let mut file = File::open(path)?;
    let opened_metadata = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.dev() != opened_metadata.dev() || metadata.ino() != opened_metadata.ino() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "save file changed while it was being opened",
            ));
        }
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let mut game = decode(&bytes)?;
    game.options.save_file = path.to_string_lossy().into_owned();
    if game.wizard {
        return Ok(game);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if opened_metadata.nlink() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "save file must have exactly one hard link",
            ));
        }
    }
    let current_metadata = fs::symlink_metadata(path)?;
    if current_metadata.file_type().is_symlink() || !current_metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "save file changed while it was being restored",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if current_metadata.dev() != opened_metadata.dev()
            || current_metadata.ino() != opened_metadata.ino()
            || current_metadata.nlink() != 1
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "save file changed while it was being restored",
            ));
        }
    }
    fs::remove_file(path)?;
    Ok(game)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_preserves_rng_and_state() {
        let p = std::env::temp_dir().join(format!("mrzavec-test-{}.json", std::process::id()));
        let mut g = Game::new(99);
        g.execute(crate::command::Command::Rest);
        g.last_command = Some('z');
        g.last_item = g.player.inventory.first().map(|item| item.id);
        g.last_direction = Some(crate::command::Direction::UpLeft);
        g.last_hand = Some(1);
        save(&g, &p).unwrap();
        let mut restored = load(&p).unwrap();
        assert_eq!(g, restored);
        assert_eq!(g.rng.next_u32(), restored.rng.next_u32());
        let _ = fs::remove_file(p);
    }

    #[test]
    fn restored_continuation_matches_uninterrupted_game() {
        let p =
            std::env::temp_dir().join(format!("mrzavec-continuation-{}.json", std::process::id()));
        let mut uninterrupted = Game::new(1234);
        for _ in 0..5 {
            uninterrupted.execute(crate::command::Command::Rest);
        }
        save(&uninterrupted, &p).unwrap();
        let mut restored = load(&p).unwrap();
        let commands = [
            crate::command::Command::Rest,
            crate::command::Command::Search,
            crate::command::Command::Move(crate::command::Direction::Right),
            crate::command::Command::Rest,
        ];
        for command in commands {
            uninterrupted.execute(command);
            restored.execute(command);
            assert_eq!(uninterrupted, restored)
        }
        let _ = fs::remove_file(p);
    }

    #[test]
    fn successful_restore_consumes_the_save_file() {
        let p = std::env::temp_dir().join(format!("mrzavec-consume-{}.json", std::process::id()));
        save(&Game::new(7), &p).unwrap();
        let restored = restore(&p).unwrap();
        assert_eq!(restored.save_version, 12);
        assert_eq!(restored.options.save_file, p.to_string_lossy());
        assert!(!p.exists());
    }

    #[test]
    fn save_does_not_reuse_the_legacy_predictable_temp_path() {
        let path =
            std::env::temp_dir().join(format!("mrzavec-safe-temp-{}.json", std::process::id()));
        let predictable = path.with_extension("tmp");
        fs::write(&predictable, b"belongs to somebody else").unwrap();

        save(&Game::new(71), &path).unwrap();

        assert_eq!(fs::read(&predictable).unwrap(), b"belongs to somebody else");
        assert_eq!(load(&path).unwrap().save_version, 12);
        fs::remove_file(predictable).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn saved_games_use_the_reference_read_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!("mrzavec-mode-{}.json", std::process::id()));
        save(&Game::new(72), &path).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o400
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn saving_again_atomically_replaces_an_existing_save() {
        let path =
            std::env::temp_dir().join(format!("mrzavec-replace-{}.json", std::process::id()));
        let mut first = Game::new(73);
        first.player.gold = 1;
        save(&first, &path).unwrap();
        let mut second = Game::new(74);
        second.player.gold = 999;
        save(&second, &path).unwrap();
        assert_eq!(load(&path).unwrap().player.gold, 999);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_restore_does_not_consume_the_file() {
        let p = std::env::temp_dir().join(format!("mrzavec-invalid-{}.json", std::process::id()));
        fs::write(&p, b"not a save").unwrap();
        assert!(restore(&p).is_err());
        assert!(p.exists());
        let _ = fs::remove_file(p);
    }

    #[test]
    fn restore_rejects_non_regular_files() {
        let directory =
            std::env::temp_dir().join(format!("mrzavec-save-directory-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let error = restore(&directory).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        fs::remove_dir(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn restore_rejects_symbolic_links_without_consuming_the_target() {
        use std::os::unix::fs::symlink;

        let target =
            std::env::temp_dir().join(format!("mrzavec-link-target-{}", std::process::id()));
        let link = target.with_extension("symlink");
        save(&Game::new(9), &target).unwrap();
        symlink(&target, &link).unwrap();
        assert!(restore(&link).is_err());
        assert!(target.exists());
        assert!(link.exists());
        fs::remove_file(link).unwrap();
        fs::remove_file(target).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn restore_rejects_multiply_linked_saves_without_consuming_them() {
        let base = std::env::temp_dir().join(format!("mrzavec-linked-{}", std::process::id()));
        let link = base.with_extension("link");
        save(&Game::new(8), &base).unwrap();
        fs::hard_link(&base, &link).unwrap();
        assert!(restore(&base).is_err());
        assert!(base.exists());
        assert!(link.exists());
        let _ = fs::remove_file(base);
        let _ = fs::remove_file(link);
    }

    #[cfg(unix)]
    #[test]
    fn wizard_restore_is_reusable_and_allows_reference_hard_links() {
        let base = std::env::temp_dir().join(format!("mrzavec-wizard-save-{}", std::process::id()));
        let link = base.with_extension("link");
        let mut game = Game::new(81);
        game.set_wizard(true);
        save(&game, &base).unwrap();
        fs::hard_link(&base, &link).unwrap();

        let restored = restore(&base).unwrap();

        assert!(restored.wizard);
        assert_eq!(restored.options.save_file, base.to_string_lossy());
        assert!(base.exists());
        assert!(link.exists());
        fs::remove_file(base).unwrap();
        fs::remove_file(link).unwrap();
    }
}
