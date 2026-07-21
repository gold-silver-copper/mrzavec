use crate::{
    Game,
    platform::{
        KeyValueStorage, StorageError, WEB_ACTIVE_SAVE_KEY, active_save_slot, logical_slot,
        save_storage_key,
    },
};
use std::io;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(not(target_arch = "wasm32"))]
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn encode_game(game: &Game) -> io::Result<Vec<u8>> {
    serde_json::to_vec(game).map_err(io::Error::other)
}

pub fn decode_game(bytes: &[u8]) -> io::Result<Game> {
    let game: Game = serde_json::from_slice(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if game.save_version != 13 {
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

pub fn save_to_storage(
    game: &Game,
    slot: &str,
    storage: &impl KeyValueStorage,
) -> Result<(), StorageError> {
    let json = serde_json::to_string(game)
        .map_err(|error| StorageError::new("encode save", error.to_string()))?;
    let previous_slot = storage.get_item(WEB_ACTIVE_SAVE_KEY)?;
    storage.set_item(WEB_ACTIVE_SAVE_KEY, logical_slot(slot))?;
    if let Err(write_error) = storage.set_item(&save_storage_key(slot), &json) {
        let rollback = match previous_slot {
            Some(previous_slot) => storage.set_item(WEB_ACTIVE_SAVE_KEY, &previous_slot),
            None => storage.remove_item(WEB_ACTIVE_SAVE_KEY),
        };
        return match rollback {
            Ok(()) => Err(write_error),
            Err(rollback_error) => Err(StorageError::new(
                "write save",
                format!("{write_error}; active-slot rollback also failed: {rollback_error}"),
            )),
        };
    }
    Ok(())
}

pub fn storage_has_save(slot: &str, storage: &impl KeyValueStorage) -> Result<bool, StorageError> {
    storage
        .get_item(&save_storage_key(slot))
        .map(|value| value.is_some())
}

pub fn restore_from_storage(
    slot: &str,
    storage: &impl KeyValueStorage,
) -> Result<Option<Game>, StorageError> {
    let key = save_storage_key(slot);
    let Some(json) = storage.get_item(&key)? else {
        return Ok(None);
    };
    let mut game = decode_game(json.as_bytes())
        .map_err(|error| StorageError::new("decode save", error.to_string()))?;
    game.options.save_file = slot.to_owned();
    if !game.wizard {
        storage.remove_item(&key)?;
    }
    Ok(Some(game))
}

pub fn restore_browser_game(
    default_slot: &str,
    storage: &impl KeyValueStorage,
) -> Result<Option<Game>, StorageError> {
    let slot = active_save_slot(storage)?
        .filter(|slot| !slot.is_empty())
        .unwrap_or_else(|| default_slot.to_owned());
    match restore_from_storage(&slot, storage) {
        Ok(Some(game)) => Ok(Some(game)),
        Ok(None) if slot == default_slot => Ok(None),
        Ok(None) => {
            storage.remove_item(WEB_ACTIVE_SAVE_KEY)?;
            restore_from_storage(default_slot, storage)
        }
        Err(error) if slot == default_slot => Err(error),
        Err(active_error) => {
            storage
                .remove_item(WEB_ACTIVE_SAVE_KEY)
                .map_err(|clear_error| {
                    StorageError::new(
                        "restore save",
                        format!(
                            "{active_error}; clearing the active slot also failed: {clear_error}"
                        ),
                    )
                })?;
            match restore_from_storage(default_slot, storage) {
                Ok(Some(mut game)) => {
                    game.message(format!(
                        "could not restore save slot {slot:?}; restored {default_slot:?} instead"
                    ));
                    Ok(Some(game))
                }
                Ok(None) => Err(active_error),
                Err(default_error) => Err(StorageError::new(
                    "restore save",
                    format!(
                        "active slot {slot:?} failed: {active_error}; fallback slot \
                         {default_slot:?} failed: {default_error}"
                    ),
                )),
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save(game: &Game, path: &Path) -> io::Result<()> {
    let bytes = encode_game(game)?;
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
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
pub fn load(path: &Path) -> io::Result<Game> {
    let bytes = fs::read(path)?;
    decode_game(&bytes)
}
#[cfg(not(target_arch = "wasm32"))]
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
    let mut game = decode_game(&bytes)?;
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
    use std::{cell::RefCell, collections::HashMap};

    #[derive(Default)]
    struct MemoryStorage {
        values: RefCell<HashMap<String, String>>,
        fail_reads: bool,
        fail_writes: bool,
        fail_save_writes: bool,
        fail_removals: bool,
    }

    impl KeyValueStorage for MemoryStorage {
        fn get_item(&self, key: &str) -> Result<Option<String>, StorageError> {
            if self.fail_reads {
                return Err(StorageError::new("read", "storage unavailable"));
            }
            Ok(self.values.borrow().get(key).cloned())
        }

        fn set_item(&self, key: &str, value: &str) -> Result<(), StorageError> {
            if self.fail_writes
                || (self.fail_save_writes && key.starts_with(crate::platform::WEB_SAVE_PREFIX))
            {
                return Err(StorageError::new("write", "quota exceeded"));
            }
            self.values
                .borrow_mut()
                .insert(key.to_owned(), value.to_owned());
            Ok(())
        }

        fn remove_item(&self, key: &str) -> Result<(), StorageError> {
            if self.fail_removals {
                return Err(StorageError::new("remove", "storage unavailable"));
            }
            self.values.borrow_mut().remove(key);
            Ok(())
        }
    }

    #[test]
    fn pure_codec_round_trips_and_rejects_corruption() {
        let game = Game::new(98);
        assert_eq!(decode_game(&encode_game(&game).unwrap()).unwrap(), game);
        assert_eq!(
            decode_game(b"not a save").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        let mut incompatible = Game::new(99);
        incompatible.save_version -= 1;
        assert_eq!(
            decode_game(&encode_game(&incompatible).unwrap())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn normal_browser_restore_is_single_use_but_wizard_restore_is_reusable() {
        let storage = MemoryStorage::default();
        save_to_storage(&Game::new(1), "campaign", &storage).unwrap();
        assert_eq!(
            storage.get_item(WEB_ACTIVE_SAVE_KEY).unwrap().as_deref(),
            Some("campaign")
        );
        assert!(storage_has_save("campaign", &storage).unwrap());
        assert!(
            restore_from_storage("campaign", &storage)
                .unwrap()
                .is_some()
        );
        assert!(!storage_has_save("campaign", &storage).unwrap());

        let mut wizard = Game::new(2);
        wizard.set_wizard(true);
        save_to_storage(&wizard, "wizard", &storage).unwrap();
        assert!(
            restore_from_storage("wizard", &storage)
                .unwrap()
                .unwrap()
                .wizard
        );
        assert!(storage_has_save("wizard", &storage).unwrap());
    }

    #[test]
    fn corrupt_browser_restore_is_not_consumed() {
        let storage = MemoryStorage::default();
        storage
            .values
            .borrow_mut()
            .insert(save_storage_key("default"), "broken".into());
        assert!(restore_from_storage("default", &storage).is_err());
        assert!(storage_has_save("default", &storage).unwrap());
    }

    #[test]
    fn corrupt_active_browser_save_falls_back_without_consuming_the_corrupt_data() {
        let mut fallback = Game::new(30);
        fallback.player.gold = 777;
        let storage = MemoryStorage {
            values: RefCell::new(HashMap::from([
                (WEB_ACTIVE_SAVE_KEY.into(), "campaign".into()),
                (save_storage_key("campaign"), "broken".into()),
                (
                    save_storage_key("default"),
                    serde_json::to_string(&fallback).unwrap(),
                ),
            ])),
            ..Default::default()
        };

        let restored = restore_browser_game("default", &storage).unwrap().unwrap();

        assert_eq!(restored.player.gold, 777);
        assert_eq!(restored.options.save_file, "default");
        assert!(
            restored
                .messages
                .last()
                .unwrap()
                .contains("restored \"default\" instead")
        );
        assert_eq!(
            storage
                .get_item(&save_storage_key("campaign"))
                .unwrap()
                .as_deref(),
            Some("broken")
        );
        assert!(!storage_has_save("default", &storage).unwrap());
        assert_eq!(storage.get_item(WEB_ACTIVE_SAVE_KEY).unwrap(), None);
    }

    #[test]
    fn browser_fallback_failure_preserves_both_save_entries() {
        let storage = MemoryStorage {
            values: RefCell::new(HashMap::from([
                (WEB_ACTIVE_SAVE_KEY.into(), "campaign".into()),
                (save_storage_key("campaign"), "broken campaign".into()),
                (save_storage_key("default"), "broken default".into()),
            ])),
            ..Default::default()
        };

        let error = restore_browser_game("default", &storage).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("active slot \"campaign\" failed")
        );
        assert!(storage_has_save("campaign", &storage).unwrap());
        assert!(storage_has_save("default", &storage).unwrap());
        assert_eq!(storage.get_item(WEB_ACTIVE_SAVE_KEY).unwrap(), None);
    }

    #[test]
    fn browser_read_and_remove_failures_are_propagated_without_data_loss() {
        let key = save_storage_key("default");
        let json = serde_json::to_string(&Game::new(3)).unwrap();
        let unavailable = MemoryStorage {
            values: RefCell::new(HashMap::from([(key.clone(), json.clone())])),
            fail_reads: true,
            ..Default::default()
        };
        assert!(restore_from_storage("default", &unavailable).is_err());
        assert_eq!(unavailable.values.borrow().get(&key).unwrap(), &json);

        let cannot_consume = MemoryStorage {
            values: RefCell::new(HashMap::from([(key.clone(), json.clone())])),
            fail_removals: true,
            ..Default::default()
        };
        assert!(restore_from_storage("default", &cannot_consume).is_err());
        assert_eq!(cannot_consume.values.borrow().get(&key).unwrap(), &json);
    }

    #[test]
    fn browser_write_failure_preserves_an_existing_save() {
        let key = save_storage_key("default");
        let storage = MemoryStorage {
            values: RefCell::new(HashMap::from([(key.clone(), "old save".into())])),
            fail_writes: true,
            ..Default::default()
        };
        assert!(save_to_storage(&Game::new(3), "default", &storage).is_err());
        assert_eq!(storage.values.borrow().get(&key).unwrap(), "old save");
    }

    #[test]
    fn browser_save_failure_restores_the_previous_active_slot() {
        let existing_key = save_storage_key("existing");
        let storage = MemoryStorage {
            values: RefCell::new(HashMap::from([
                (WEB_ACTIVE_SAVE_KEY.into(), "existing".into()),
                (existing_key.clone(), "old save".into()),
            ])),
            fail_save_writes: true,
            ..Default::default()
        };

        assert!(save_to_storage(&Game::new(4), "new", &storage).is_err());
        assert_eq!(
            storage.get_item(WEB_ACTIVE_SAVE_KEY).unwrap().as_deref(),
            Some("existing")
        );
        assert_eq!(
            storage.values.borrow().get(&existing_key).unwrap(),
            "old save"
        );
        assert!(
            !storage
                .values
                .borrow()
                .contains_key(&save_storage_key("new"))
        );
    }

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
        assert_eq!(restored.save_version, 13);
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
        assert_eq!(load(&path).unwrap().save_version, 13);
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
