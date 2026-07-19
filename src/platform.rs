#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};
use std::{error::Error, fmt};

pub const WEB_SAVE_PREFIX: &str = "mrzavec.save.v12";
pub const WEB_SCORE_PREFIX: &str = "mrzavec.scores.v1";
pub const WEB_ACTIVE_SAVE_KEY: &str = "mrzavec.save.active";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError {
    operation: &'static str,
    detail: String,
}

impl StorageError {
    pub fn new(operation: &'static str, detail: impl Into<String>) -> Self {
        Self {
            operation,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "browser storage {} failed: {}",
            self.operation, self.detail
        )
    }
}

impl Error for StorageError {}

/// The small key/value surface required by the web save and score backends.
/// Keeping it as a trait makes quota, privacy-mode, and disabled-storage
/// failures testable on native targets.
pub trait KeyValueStorage {
    fn get_item(&self, key: &str) -> Result<Option<String>, StorageError>;
    fn set_item(&self, key: &str, value: &str) -> Result<(), StorageError>;
    fn remove_item(&self, key: &str) -> Result<(), StorageError>;
}

pub fn save_storage_key(slot: &str) -> String {
    storage_key(WEB_SAVE_PREFIX, slot)
}

pub fn score_storage_key(slot: &str) -> String {
    storage_key(WEB_SCORE_PREFIX, slot)
}

pub fn active_save_slot(storage: &impl KeyValueStorage) -> Result<Option<String>, StorageError> {
    storage.get_item(WEB_ACTIVE_SAVE_KEY)
}

pub fn unix_time_seconds() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    return SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    #[cfg(target_arch = "wasm32")]
    return (js_sys::Date::now() / 1000.0).max(0.0) as u64;
}

pub fn random_seed() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    return SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |duration| duration.as_nanos() as u64);
    #[cfg(target_arch = "wasm32")]
    return js_sys::Date::now().to_bits();
}

fn storage_key(prefix: &str, slot: &str) -> String {
    let slot = logical_slot(slot);
    format!("{prefix}:{slot}")
}

pub(crate) fn logical_slot(slot: &str) -> &str {
    if slot.is_empty() { "default" } else { slot }
}

#[cfg(target_arch = "wasm32")]
pub struct LocalStorage(web_sys::Storage);

#[cfg(target_arch = "wasm32")]
impl LocalStorage {
    pub fn open() -> Result<Self, StorageError> {
        let window =
            web_sys::window().ok_or_else(|| StorageError::new("open", "window is unavailable"))?;
        let storage = window
            .local_storage()
            .map_err(|error| StorageError::new("open", format!("{error:?}")))?
            .ok_or_else(|| StorageError::new("open", "localStorage is unavailable"))?;
        Ok(Self(storage))
    }
}

#[cfg(target_arch = "wasm32")]
impl KeyValueStorage for LocalStorage {
    fn get_item(&self, key: &str) -> Result<Option<String>, StorageError> {
        self.0
            .get_item(key)
            .map_err(|error| StorageError::new("read", format!("{error:?}")))
    }

    fn set_item(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.0
            .set_item(key, value)
            .map_err(|error| StorageError::new("write", format!("{error:?}")))
    }

    fn remove_item(&self, key: &str) -> Result<(), StorageError> {
        self.0
            .remove_item(key)
            .map_err(|error| StorageError::new("remove", format!("{error:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_keys_are_versioned_and_slots_are_logical() {
        assert_eq!(save_storage_key("default"), "mrzavec.save.v12:default");
        assert_eq!(score_storage_key("local"), "mrzavec.scores.v1:local");
        assert_eq!(save_storage_key(""), "mrzavec.save.v12:default");
        assert_eq!(save_storage_key("slot two"), "mrzavec.save.v12:slot two");
    }
}
