use directories::ProjectDirs;
use std::path::PathBuf;

/// XDG-compliant paths for rec data, config, and state.
///
/// On Linux, uses the XDG Base Directory Specification:
/// - Data: ~/.local/share/rec/sessions/
/// - Config: ~/.config/rec/config.toml
/// - State: ~/.local/state/rec/ (or ~/.local/share/rec/state)
///
/// If XDG directories are unavailable (`ProjectDirs` returns None),
/// falls back to ~/.rec for all paths.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Directory for session data files
    pub data_dir: PathBuf,

    /// Directory for configuration files
    pub config_dir: PathBuf,

    /// Path to the main config file
    pub config_file: PathBuf,

    /// Directory for state files (recording state, PID files)
    pub state_dir: PathBuf,
}

impl Paths {
    /// Create new Paths using XDG directories with ~/.rec fallback.
    ///
    /// Attempts to use the XDG Base Directory Specification via the
    /// `directories` crate. If that fails (e.g., on systems without
    /// proper XDG support), falls back to ~/.rec for all paths.
    ///
    /// # Panics
    ///
    /// Panics if the home directory cannot be determined (XDG fallback path).
    #[must_use]
    pub fn new() -> Self {
        // Try XDG first via directories crate
        if let Some(proj_dirs) = ProjectDirs::from("", "", "rec") {
            Self {
                data_dir: proj_dirs.data_dir().join("sessions"),
                config_dir: proj_dirs.config_dir().to_path_buf(),
                config_file: proj_dirs.config_dir().join("config.toml"),
                state_dir: proj_dirs.state_dir().map_or_else(
                    || proj_dirs.data_dir().join("state"),
                    std::path::Path::to_path_buf,
                ),
            }
        } else {
            // Fallback to ~/.rec
            let home = directories::BaseDirs::new()
                .expect("Could not determine home directory")
                .home_dir()
                .to_path_buf();
            let rec_dir = home.join(".rec");
            Self {
                data_dir: rec_dir.join("sessions"),
                config_dir: rec_dir.clone(),
                config_file: rec_dir.join("config.toml"),
                state_dir: rec_dir.join("state"),
            }
        }
    }

    /// Ensure all directories exist, creating them if necessary.
    ///
    /// Creates:
    /// - `data_dir` (for session files)
    /// - `config_dir` (for config.toml)
    /// - `state_dir` (for recording state)
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation fails (e.g., permission denied).
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.state_dir)?;
        Ok(())
    }

    /// Get the path for a session file by ID.
    ///
    /// Session files use the `.ndjson` extension for NDJSON format.
    #[must_use]
    pub fn session_file(&self, id: &str) -> PathBuf {
        self.data_dir.join(format!("{id}.ndjson"))
    }

    /// Get the path for a session backup file by ID.
    ///
    /// Backup files use the `.ndjson.bak` extension and are created
    /// before modifying session data (rename, tag, etc.) to allow
    /// recovery if the modification fails.
    #[must_use]
    pub fn backup_file(&self, id: &str) -> PathBuf {
        self.data_dir.join(format!("{id}.ndjson.bak"))
    }
}

impl Default for Paths {
    fn default() -> Self {
        Self::new()
    }
}

/// Global paths instance (lazy initialization).
///
/// Returns a static reference to the Paths instance, which is
/// created once on first access using `OnceLock`.
pub fn get_paths() -> &'static Paths {
    use std::sync::OnceLock;
    static PATHS: OnceLock<Paths> = OnceLock::new();
    PATHS.get_or_init(Paths::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_new() {
        let paths = Paths::new();

        // Basic sanity checks - paths should not be empty
        assert!(!paths.data_dir.as_os_str().is_empty());
        assert!(!paths.config_dir.as_os_str().is_empty());
        assert!(!paths.config_file.as_os_str().is_empty());
        assert!(!paths.state_dir.as_os_str().is_empty());
    }

    #[test]
    fn test_paths_session_file() {
        let paths = Paths::new();
        let session_path = paths.session_file("test-session-123");

        assert!(session_path.to_string_lossy().contains("test-session-123"));
        assert!(session_path.extension().is_some_and(|ext| ext == "ndjson"));
    }

    #[test]
    fn test_get_paths_returns_same_instance() {
        let paths1 = get_paths();
        let paths2 = get_paths();

        // Should be the same reference
        assert!(std::ptr::eq(paths1, paths2));
    }

    #[test]
    fn test_paths_xdg_structure() {
        let paths = Paths::new();

        // On Linux with XDG support, paths should follow XDG structure
        // The data_dir should end with "sessions"
        assert!(
            paths
                .data_dir
                .file_name()
                .is_some_and(|name| name == "sessions")
        );

        // The config_file should be config.toml
        assert!(
            paths
                .config_file
                .file_name()
                .is_some_and(|name| name == "config.toml")
        );
    }
}
