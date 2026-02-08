//! Session resolution by name-first, UUID-fallback.
//!
//! Provides a reusable helper to resolve a session identifier (which may be
//! a human-readable name or a UUID) to a loaded `Session`. Used by all
//! session-accepting commands (replay, show, delete, rename, edit, tag, export).

use crate::error::RecError;
use crate::models::Session;
use crate::session::fuzzy;
use crate::storage::{AliasStore, SessionStore};

/// Error details when session resolution fails, including fuzzy suggestions.
#[derive(Debug)]
pub struct ResolveError {
    /// The underlying error.
    pub error: RecError,
    /// Fuzzy suggestions for similar session names (empty if none found).
    pub suggestions: Vec<fuzzy::Suggestion>,
}

/// Resolve a session identifier to a loaded `Session`.
///
/// Resolution order:
/// 1. Scan all sessions for a name match (collecting names for fuzzy fallback)
/// 2. If exactly one name match: return it
/// 3. If no name match: try UUID lookup via `store.load(identifier)`
/// 4. If no UUID match either: return `ResolveError` with fuzzy suggestions
/// 5. If multiple name matches AND `interactive`: enhanced `dialoguer::Select`
///    with date + command count
/// 6. If multiple name matches AND NOT `interactive`: pick most recent by
///    `started_at` (deterministic duplicate resolution)
///
/// # Errors
///
/// Returns `ResolveError` containing a `RecError` and optional fuzzy suggestions.
///
/// # Panics
///
/// Panics if internal iterator operations produce an empty collection when
/// a non-empty result is expected (should not happen due to match guards).
pub fn resolve_session(
    store: &SessionStore,
    identifier: &str,
    interactive: bool,
) -> std::result::Result<Session, ResolveError> {
    let all_ids = store.list().map_err(|e| ResolveError {
        error: e,
        suggestions: Vec::new(),
    })?;

    // Search by name, collecting all (name, uuid) pairs for fuzzy fallback
    let mut name_matches = Vec::new();
    let mut all_names: Vec<(String, String)> = Vec::new();
    for id in &all_ids {
        if let Ok(s) = store.load(id) {
            all_names.push((s.name().to_string(), id.clone()));
            if s.name() == identifier {
                name_matches.push(s);
            }
        }
    }

    match name_matches.len() {
        1 => Ok(name_matches.into_iter().next().unwrap()),
        0 => {
            // Try UUID lookup
            if let Ok(s) = store.load(identifier) {
                Ok(s)
            } else {
                // UUID lookup also failed — compute fuzzy suggestions
                let suggestions = fuzzy::suggest_sessions(identifier, &all_names, 3);
                Err(ResolveError {
                    error: RecError::SessionNotFound(identifier.to_string()),
                    suggestions,
                })
            }
        }
        _ => {
            // Multiple matches
            if interactive {
                let items: Vec<String> = name_matches
                    .iter()
                    .map(|s| {
                        let date = chrono::DateTime::from_timestamp(s.header.started_at as i64, 0)
                            .map_or_else(
                                || "unknown".to_string(),
                                |dt| {
                                    let local: chrono::DateTime<chrono::Local> = dt.into();
                                    local.format("%b %d").to_string()
                                },
                            );
                        let cmd_count = s.commands.len();
                        format!("{} ({}, {} commands)", s.name(), date, cmd_count)
                    })
                    .collect();

                let selection = dialoguer::Select::new()
                    .with_prompt("Multiple sessions found. Select one")
                    .items(&items)
                    .default(0)
                    .interact()
                    .map_err(|_| ResolveError {
                        error: RecError::InvalidSession("Session selection cancelled".to_string()),
                        suggestions: Vec::new(),
                    })?;

                Ok(name_matches.into_iter().nth(selection).unwrap())
            } else {
                // Non-interactive: pick most recent by started_at
                name_matches.sort_by(|a, b| {
                    b.header
                        .started_at
                        .partial_cmp(&a.header.started_at)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                Ok(name_matches.into_iter().next().unwrap())
            }
        }
    }
}

/// Resolve a session identifier with alias lookup.
///
/// Resolution order:
/// 1. Check alias store -- if alias exists, resolve the aliased target
/// 2. Fall through to normal resolution (name -> UUID)
///
/// This is the preferred entry point for all commands that accept
/// a session identifier. The non-alias version `resolve_session`
/// remains available for internal use.
///
/// # Errors
///
/// Returns `ResolveError` if the session cannot be found by alias, name, or UUID.
pub fn resolve_session_with_alias(
    store: &SessionStore,
    alias_store: &AliasStore,
    identifier: &str,
    interactive: bool,
) -> std::result::Result<Session, ResolveError> {
    // Step 1: Check aliases
    if let Ok(Some(target)) = alias_store.get(identifier) {
        // Resolve the aliased target through normal resolution
        return resolve_session(store, &target, interactive);
    }

    // Step 2: Normal resolution (name -> UUID)
    resolve_session(store, identifier, interactive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Command, SessionStatus};
    use crate::storage::Paths;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_paths(temp_dir: &TempDir) -> Paths {
        Paths {
            data_dir: temp_dir.path().join("sessions"),
            config_dir: temp_dir.path().join("config"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            state_dir: temp_dir.path().join("state"),
        }
    }

    fn create_test_session(name: &str) -> Session {
        let mut session = Session::new(name);
        session.commands.push(Command::new(
            0,
            "echo hello".to_string(),
            PathBuf::from("/tmp"),
        ));
        session.complete(SessionStatus::Completed);
        session
    }

    /// Create a test session with a specific `started_at` timestamp.
    fn create_test_session_at(name: &str, started_at: f64) -> Session {
        let mut session = create_test_session(name);
        session.header.started_at = started_at;
        session
    }

    #[test]
    fn test_resolve_by_name() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let session = create_test_session("my-session");
        store.save(&session).unwrap();

        let resolved = resolve_session(&store, "my-session", false).unwrap();
        assert_eq!(resolved.name(), "my-session");
        assert_eq!(resolved.id(), session.id());
    }

    #[test]
    fn test_resolve_by_uuid() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let session = create_test_session("my-session");
        let id = session.id().to_string();
        store.save(&session).unwrap();

        let resolved = resolve_session(&store, &id, false).unwrap();
        assert_eq!(resolved.name(), "my-session");
        assert_eq!(resolved.id().to_string(), id);
    }

    #[test]
    fn test_resolve_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        let result = resolve_session(&store, "nonexistent", false);
        assert!(result.is_err());
        let resolve_err = result.unwrap_err();
        match &resolve_err.error {
            RecError::SessionNotFound(name) => assert_eq!(name, "nonexistent"),
            _ => panic!(
                "Expected SessionNotFound error, got: {:?}",
                resolve_err.error
            ),
        }
    }

    #[test]
    fn test_resolve_multiple_matches_non_interactive_picks_most_recent() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        // Create two sessions with the same name at different times
        let older = create_test_session_at("duplicate", 1000.0);
        let newer = create_test_session_at("duplicate", 2000.0);
        let newer_id = newer.id();
        store.save(&older).unwrap();
        store.save(&newer).unwrap();

        // Non-interactive should pick the most recent (newer)
        let result = resolve_session(&store, "duplicate", false);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let resolved = result.unwrap();
        assert_eq!(resolved.id(), newer_id);
        assert_eq!(resolved.name(), "duplicate");
    }

    #[test]
    fn test_resolve_not_found_with_fuzzy_suggestions() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        // Create a session with a similar name
        let session = create_test_session("deploy-v1");
        store.save(&session).unwrap();

        // Search for a typo
        let result = resolve_session(&store, "deply-v1", false);
        assert!(result.is_err());
        let resolve_err = result.unwrap_err();
        match &resolve_err.error {
            RecError::SessionNotFound(name) => assert_eq!(name, "deply-v1"),
            _ => panic!("Expected SessionNotFound error"),
        }
        assert!(
            !resolve_err.suggestions.is_empty(),
            "Expected fuzzy suggestions for typo"
        );
        assert_eq!(resolve_err.suggestions[0].name, "deploy-v1");
    }

    #[test]
    fn test_resolve_not_found_no_suggestions() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths);

        // Create a session with a very different name
        let session = create_test_session("deploy-v1");
        store.save(&session).unwrap();

        // Search for something completely unrelated
        let result = resolve_session(&store, "zzzzzzzzz", false);
        assert!(result.is_err());
        let resolve_err = result.unwrap_err();
        match &resolve_err.error {
            RecError::SessionNotFound(name) => assert_eq!(name, "zzzzzzzzz"),
            _ => panic!("Expected SessionNotFound error"),
        }
        assert!(
            resolve_err.suggestions.is_empty(),
            "Expected no suggestions for completely unrelated query"
        );
    }

    #[test]
    fn test_resolve_with_alias() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("prod-deploy-v3");
        let session_id = session.id();
        store.save(&session).unwrap();

        // Create alias pointing to the session name
        let alias_store = AliasStore::new(&paths);
        alias_store.set("deploy", "prod-deploy-v3").unwrap();

        // Resolve by alias name
        let resolved = resolve_session_with_alias(&store, &alias_store, "deploy", false).unwrap();
        assert_eq!(resolved.name(), "prod-deploy-v3");
        assert_eq!(resolved.id(), session_id);
    }

    #[test]
    fn test_resolve_alias_not_found_falls_through() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        let session = create_test_session("my-session");
        store.save(&session).unwrap();

        // No alias set — should fall through to normal name resolution
        let alias_store = AliasStore::new(&paths);
        let resolved =
            resolve_session_with_alias(&store, &alias_store, "my-session", false).unwrap();
        assert_eq!(resolved.name(), "my-session");
    }

    #[test]
    fn test_resolve_alias_target_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let paths = create_test_paths(&temp_dir);
        let store = SessionStore::new(paths.clone());

        // Create alias pointing to nonexistent session
        let alias_store = AliasStore::new(&paths);
        alias_store.set("broken", "nonexistent-session").unwrap();

        let result = resolve_session_with_alias(&store, &alias_store, "broken", false);
        assert!(result.is_err());
        let resolve_err = result.unwrap_err();
        match &resolve_err.error {
            RecError::SessionNotFound(name) => assert_eq!(name, "nonexistent-session"),
            _ => panic!(
                "Expected SessionNotFound error, got: {:?}",
                resolve_err.error
            ),
        }
    }
}
