//! Alias storage with atomic JSON persistence.
//!
//! Provides CRUD operations for session aliases — short names that map
//! to session names. Aliases are stored as JSON in the data directory
//! with atomic write-tmp-rename for safe persistence.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::storage::Paths;
use crate::storage::session_store::set_restrictive_permissions;

/// Serializable alias map (`alias_name` → `session_name`).
#[derive(Serialize, Deserialize, Default)]
struct AliasMap {
    aliases: HashMap<String, String>,
}

/// Persistent alias storage backed by a JSON file.
///
/// Aliases are stored at `~/.local/share/rec/aliases.json` (the parent
/// of the sessions data directory). All writes use atomic tmp-rename
/// to prevent corruption.
pub struct AliasStore {
    path: PathBuf,
}

impl AliasStore {
    /// Create a new `AliasStore` using the given paths.
    ///
    /// The alias file is placed in the parent of `data_dir` (which points
    /// to `~/.local/share/rec/sessions/`), giving `~/.local/share/rec/aliases.json`.
    #[must_use]
    pub fn new(paths: &Paths) -> Self {
        let path = paths
            .data_dir
            .parent()
            .unwrap_or(&paths.data_dir)
            .join("aliases.json");
        Self { path }
    }

    /// Look up an alias by name.
    ///
    /// Returns `Ok(None)` if the alias doesn't exist or the file hasn't
    /// been created yet.
    ///
    /// # Errors
    ///
    /// Returns an error if the alias file exists but cannot be read or parsed.
    pub fn get(&self, alias: &str) -> Result<Option<String>> {
        let map = self.load_map()?;
        Ok(map.aliases.get(alias).cloned())
    }

    /// Set an alias to point to a session name.
    ///
    /// Overwrites any existing alias with the same name.
    ///
    /// # Errors
    ///
    /// Returns an error if the alias file cannot be read or written.
    pub fn set(&self, alias: &str, session: &str) -> Result<()> {
        let mut map = self.load_map()?;
        map.aliases.insert(alias.to_string(), session.to_string());
        self.save_map(&map)
    }

    /// Remove an alias by name.
    ///
    /// Returns `Ok(true)` if the alias was removed, `Ok(false)` if it
    /// didn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the alias file cannot be read or written.
    pub fn remove(&self, alias: &str) -> Result<bool> {
        let mut map = self.load_map()?;
        let removed = map.aliases.remove(alias).is_some();
        if removed {
            self.save_map(&map)?;
        }
        Ok(removed)
    }

    /// List all aliases sorted alphabetically by alias name.
    ///
    /// Returns pairs of (`alias_name`, `session_name`).
    ///
    /// # Errors
    ///
    /// Returns an error if the alias file cannot be read or parsed.
    pub fn list(&self) -> Result<Vec<(String, String)>> {
        let map = self.load_map()?;
        let mut entries: Vec<(String, String)> = map.aliases.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(entries)
    }

    /// Load the alias map from disk.
    ///
    /// Returns an empty map if the file doesn't exist yet.
    fn load_map(&self) -> Result<AliasMap> {
        if !self.path.exists() {
            return Ok(AliasMap::default());
        }
        let content = fs::read_to_string(&self.path)?;
        let map: AliasMap = serde_json::from_str(&content)?;
        Ok(map)
    }

    /// Save the alias map to disk atomically.
    ///
    /// Writes to a temporary file first, then renames to the target path.
    /// This prevents corruption if the process is interrupted mid-write.
    fn save_map(&self, map: &AliasMap) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = self.path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(map)?;
        fs::write(&tmp_path, content)?;
        fs::rename(&tmp_path, &self.path)?;

        // Set restrictive permissions (0o600) to prevent other users from reading
        set_restrictive_permissions(&self.path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_store(temp_dir: &TempDir) -> AliasStore {
        let paths = Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        };
        AliasStore::new(&paths)
    }

    #[test]
    fn test_alias_set_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        store.set("deploy", "session-2026-01-15-deploy").unwrap();
        let result = store.get("deploy").unwrap();
        assert_eq!(result, Some("session-2026-01-15-deploy".to_string()));
    }

    #[test]
    fn test_alias_get_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let result = store.get("nonexistent").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_alias_remove() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        store.set("deploy", "session-deploy").unwrap();
        let removed = store.remove("deploy").unwrap();
        assert!(removed);

        let result = store.get("deploy").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_alias_remove_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let removed = store.remove("nonexistent").unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_alias_list_sorted() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        store.set("zebra", "session-z").unwrap();
        store.set("alpha", "session-a").unwrap();
        store.set("middle", "session-m").unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], ("alpha".to_string(), "session-a".to_string()));
        assert_eq!(list[1], ("middle".to_string(), "session-m".to_string()));
        assert_eq!(list[2], ("zebra".to_string(), "session-z".to_string()));
    }

    #[test]
    fn test_alias_list_empty() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        let list = store.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_alias_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        store.set("deploy", "session-old").unwrap();
        store.set("deploy", "session-new").unwrap();

        let result = store.get("deploy").unwrap();
        assert_eq!(result, Some("session-new".to_string()));

        // Should still be only one entry
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_alias_atomic_write() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        store.set("test", "session-test").unwrap();

        // Verify the tmp file doesn't linger
        let tmp_path = store.path.with_extension("json.tmp");
        assert!(
            !tmp_path.exists(),
            "Temporary file should not exist after successful write"
        );

        // Verify the actual file exists and is valid JSON
        assert!(store.path.exists(), "Alias file should exist");
        let content = fs::read_to_string(&store.path).unwrap();
        let _: AliasMap = serde_json::from_str(&content).expect("Should be valid JSON");
    }

    #[test]
    fn test_alias_persistence_across_instances() {
        let temp_dir = TempDir::new().unwrap();

        // Write with one instance
        {
            let store = create_test_store(&temp_dir);
            store.set("persist", "session-persist").unwrap();
        }

        // Read with a new instance
        {
            let store = create_test_store(&temp_dir);
            let result = store.get("persist").unwrap();
            assert_eq!(result, Some("session-persist".to_string()));
        }
    }

    #[test]
    fn test_alias_store_path_location() {
        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        // Path should be in the parent of data_dir (which is sessions/)
        // So it should be at temp_dir/aliases.json
        assert_eq!(store.path, temp_dir.path().join("aliases.json"));
    }

    #[test]
    #[cfg(unix)]
    fn test_alias_file_has_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let store = create_test_store(&temp_dir);

        store.set("deploy", "session-deploy").unwrap();

        // Verify permissions are 0o600 (read/write for owner only)
        let metadata = fs::metadata(&store.path).unwrap();
        let mode = metadata.permissions().mode();

        // On Unix, mode includes file type bits. We only care about permission bits (lower 9 bits)
        let permission_bits = mode & 0o777;
        assert_eq!(
            permission_bits, 0o600,
            "Alias file should have 0o600 permissions, got 0o{permission_bits:o}"
        );
    }
}
