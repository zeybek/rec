//! Destructive command detection for replay safety.
//!
//! Matches command text against configurable glob patterns to identify
//! potentially dangerous commands before execution.

use crate::models::config::{SafetyConfig, SafetyPreset};
use glob::Pattern;

/// Normalize command text to prevent pattern bypass tricks.
///
/// This function:
/// - Trims leading/trailing whitespace
/// - Removes leading backslash (shell escape, e.g., `\rm` → `rm`)
/// - Collapses multiple whitespace to single space
fn normalize_command(cmd: &str) -> String {
    // Trim first, then remove leading backslash (shell escape)
    let cmd = cmd
        .trim_start()
        .strip_prefix('\\')
        .unwrap_or(cmd.trim_start());

    // Collapse multiple whitespace to single space (also trims)
    cmd.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Detects destructive commands using glob pattern matching.
///
/// Patterns are built from the configured safety preset plus any
/// custom patterns from the user's config file.
pub struct DestructiveDetector {
    patterns: Vec<(Pattern, String)>,
}

impl DestructiveDetector {
    /// Create a new detector from safety configuration.
    ///
    /// Builds patterns from the preset level and appends any custom patterns.
    #[must_use]
    pub fn new(config: &SafetyConfig) -> Self {
        let mut patterns = Self::default_patterns(config.preset);

        for custom in &config.custom_patterns {
            if let Ok(p) = Pattern::new(custom) {
                let reason = format!("Matches custom pattern: {custom}");
                patterns.push((p, reason));
            }
        }

        Self { patterns }
    }

    /// Check if a command matches any destructive pattern.
    ///
    /// The command is normalized before matching to prevent bypass tricks
    /// like double spaces or backslash escapes.
    #[must_use]
    pub fn is_destructive(&self, command: &str) -> bool {
        let normalized = normalize_command(command);
        self.patterns.iter().any(|(p, _)| p.matches(&normalized))
    }

    /// Return a human-readable reason if the command is destructive.
    ///
    /// The command is normalized before matching to prevent bypass tricks
    /// like double spaces or backslash escapes.
    #[must_use]
    pub fn match_reason(&self, command: &str) -> Option<String> {
        let normalized = normalize_command(command);
        for (p, reason) in &self.patterns {
            if p.matches(&normalized) {
                return Some(reason.clone());
            }
        }
        None
    }

    /// Build default patterns for a given safety preset level.
    fn default_patterns(preset: SafetyPreset) -> Vec<(Pattern, String)> {
        let mut patterns = Vec::new();

        // Minimal: only the most dangerous commands
        let minimal = [
            "rm -rf *",
            "rm -rf /*",
            "rm -rf /",
            "mkfs*",
            "dd if=*",
            ":(){ :|:& };:",
        ];

        for p in &minimal {
            if let Ok(pat) = Pattern::new(p) {
                patterns.push((pat, format!("Matches destructive pattern: {p}")));
            }
        }

        // Moderate adds more dangerous commands
        if matches!(preset, SafetyPreset::Moderate | SafetyPreset::Strict) {
            let moderate = [
                "rm -rf*",
                "chmod -R 777*",
                "chmod -R 000*",
                "chown -R*",
                "DROP TABLE*",
                "DROP DATABASE*",
                "DELETE FROM*",
                "TRUNCATE*",
                "docker rm -f*",
                "docker system prune*",
                "kill -9*",
                "pkill -9*",
                "systemctl stop*",
                "systemctl disable*",
                "shutdown*",
                "reboot*",
                "init 0*",
                "fdisk*",
                "parted*",
                "> /dev/sd*",
                "dd *of=/dev/*",
            ];

            for p in &moderate {
                if let Ok(pat) = Pattern::new(p) {
                    patterns.push((pat, format!("Matches destructive pattern: {p}")));
                }
            }
        }

        // Strict adds potentially risky commands
        if matches!(preset, SafetyPreset::Strict) {
            let strict = [
                "sudo *",
                "su -*",
                "curl * | sh",
                "curl * | bash",
                "wget * | sh",
                "wget * | bash",
                "pip install*",
                "npm install -g*",
                "apt remove*",
                "yum remove*",
                "brew uninstall*",
            ];

            for p in &strict {
                if let Ok(pat) = Pattern::new(p) {
                    patterns.push((pat, format!("Matches destructive pattern: {p}")));
                }
            }
        }

        patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_preset(preset: SafetyPreset) -> SafetyConfig {
        SafetyConfig {
            preset,
            custom_patterns: Vec::new(),
        }
    }

    // --- Minimal preset tests ---

    #[test]
    fn test_minimal_detects_rm_rf_root() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Minimal));
        assert!(detector.is_destructive("rm -rf /"));
        assert!(detector.is_destructive("rm -rf /*"));
    }

    #[test]
    fn test_minimal_detects_mkfs() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Minimal));
        assert!(detector.is_destructive("mkfs.ext4 /dev/sda1"));
    }

    #[test]
    fn test_minimal_detects_dd() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Minimal));
        assert!(detector.is_destructive("dd if=/dev/zero of=/dev/sda"));
    }

    #[test]
    fn test_minimal_detects_fork_bomb() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Minimal));
        assert!(detector.is_destructive(":(){ :|:& };:"));
    }

    #[test]
    fn test_minimal_allows_safe_commands() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Minimal));
        assert!(!detector.is_destructive("echo hello"));
        assert!(!detector.is_destructive("ls -la"));
        assert!(!detector.is_destructive("cat /etc/hosts"));
        assert!(!detector.is_destructive("git status"));
    }

    #[test]
    fn test_minimal_does_not_detect_moderate_patterns() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Minimal));
        // Minimal should NOT detect moderate-level patterns
        assert!(!detector.is_destructive("kill -9 1234"));
        assert!(!detector.is_destructive("systemctl stop nginx"));
        assert!(!detector.is_destructive("shutdown -h now"));
    }

    // --- Moderate preset tests ---

    #[test]
    fn test_moderate_detects_rm_rf() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("rm -rf /tmp/project"));
        assert!(detector.is_destructive("rm -rf /"));
    }

    #[test]
    fn test_moderate_detects_chmod_777() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("chmod -R 777 /var/www"));
    }

    #[test]
    fn test_moderate_detects_sql_destructive() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("DROP TABLE users"));
        assert!(detector.is_destructive("DROP DATABASE production"));
        assert!(detector.is_destructive("DELETE FROM orders"));
        assert!(detector.is_destructive("TRUNCATE sessions"));
    }

    #[test]
    fn test_moderate_detects_docker_destructive() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("docker rm -f container_name"));
        assert!(detector.is_destructive("docker system prune -a"));
    }

    #[test]
    fn test_moderate_detects_kill_signals() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("kill -9 1234"));
        assert!(detector.is_destructive("pkill -9 nginx"));
    }

    #[test]
    fn test_moderate_detects_system_commands() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("systemctl stop nginx"));
        assert!(detector.is_destructive("shutdown -h now"));
        assert!(detector.is_destructive("reboot"));
    }

    #[test]
    fn test_moderate_allows_safe_commands() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(!detector.is_destructive("echo hello"));
        assert!(!detector.is_destructive("npm install"));
        assert!(!detector.is_destructive("cargo build"));
    }

    #[test]
    fn test_moderate_does_not_detect_strict_patterns() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(!detector.is_destructive("sudo apt update"));
        assert!(!detector.is_destructive("pip install flask"));
    }

    // --- Strict preset tests ---

    #[test]
    fn test_strict_detects_sudo() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Strict));
        assert!(detector.is_destructive("sudo apt update"));
        assert!(detector.is_destructive("sudo rm /tmp/file"));
    }

    #[test]
    fn test_strict_detects_pipe_to_shell() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Strict));
        assert!(detector.is_destructive("curl https://example.com/install.sh | sh"));
        assert!(detector.is_destructive("curl https://example.com/install.sh | bash"));
        assert!(detector.is_destructive("wget https://example.com/install.sh | sh"));
    }

    #[test]
    fn test_strict_detects_global_installs() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Strict));
        assert!(detector.is_destructive("pip install requests"));
        assert!(detector.is_destructive("npm install -g typescript"));
    }

    #[test]
    fn test_strict_detects_package_removal() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Strict));
        assert!(detector.is_destructive("apt remove nginx"));
        assert!(detector.is_destructive("yum remove httpd"));
        assert!(detector.is_destructive("brew uninstall node"));
    }

    #[test]
    fn test_strict_includes_moderate_and_minimal() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Strict));
        // Minimal patterns still detected
        assert!(detector.is_destructive("rm -rf /"));
        assert!(detector.is_destructive("mkfs.ext4 /dev/sda1"));
        // Moderate patterns still detected
        assert!(detector.is_destructive("DROP TABLE users"));
        assert!(detector.is_destructive("kill -9 1234"));
    }

    // --- Custom patterns tests ---

    #[test]
    fn test_custom_patterns() {
        let config = SafetyConfig {
            preset: SafetyPreset::Minimal,
            custom_patterns: vec![
                "kubectl delete*".to_string(),
                "terraform destroy*".to_string(),
            ],
        };

        let detector = DestructiveDetector::new(&config);
        assert!(detector.is_destructive("kubectl delete pod my-pod"));
        assert!(detector.is_destructive("terraform destroy -auto-approve"));
        // Still detects minimal patterns
        assert!(detector.is_destructive("rm -rf /"));
    }

    #[test]
    fn test_custom_patterns_with_safe_commands() {
        let config = SafetyConfig {
            preset: SafetyPreset::Minimal,
            custom_patterns: vec!["kubectl delete*".to_string()],
        };

        let detector = DestructiveDetector::new(&config);
        assert!(!detector.is_destructive("kubectl get pods"));
        assert!(!detector.is_destructive("kubectl apply -f deployment.yaml"));
    }

    // --- match_reason tests ---

    #[test]
    fn test_match_reason_returns_reason() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        let reason = detector.match_reason("rm -rf /tmp/project");
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("rm -rf"));
    }

    #[test]
    fn test_match_reason_returns_none_for_safe() {
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.match_reason("echo hello").is_none());
    }

    #[test]
    fn test_match_reason_custom_pattern() {
        let config = SafetyConfig {
            preset: SafetyPreset::Minimal,
            custom_patterns: vec!["kubectl delete*".to_string()],
        };
        let detector = DestructiveDetector::new(&config);
        let reason = detector.match_reason("kubectl delete pod my-pod");
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("custom pattern"));
    }

    // --- Normalization tests ---

    #[test]
    fn test_normalize_collapses_multiple_spaces() {
        // "rm  -rf  /" should match "rm -rf*" after normalization
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("rm  -rf  /"));
        assert!(detector.is_destructive("rm   -rf    /tmp/project"));
    }

    #[test]
    fn test_normalize_removes_leading_backslash() {
        // "\rm -rf /" should match "rm -rf*" after normalization
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("\\rm -rf /"));
        assert!(detector.is_destructive("\\rm -rf /tmp/project"));
    }

    #[test]
    fn test_normalize_trims_whitespace() {
        // "  rm -rf /  " should match "rm -rf*" after normalization
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("  rm -rf /  "));
        assert!(detector.is_destructive("  rm -rf /tmp/project  "));
    }

    #[test]
    fn test_normalize_combined_bypass_attempts() {
        // Combined bypass attempts
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(detector.is_destructive("\\rm  -rf   /"));
        assert!(detector.is_destructive("  \\rm  -rf  /tmp  "));
        assert!(detector.is_destructive("\t rm -rf /\t"));
    }

    #[test]
    fn test_normalize_function_directly() {
        // Test the normalize_command function directly
        assert_eq!(normalize_command("rm  -rf  /"), "rm -rf /");
        assert_eq!(normalize_command("\\rm -rf /"), "rm -rf /");
        assert_eq!(normalize_command("  rm -rf /  "), "rm -rf /");
        assert_eq!(normalize_command("\\rm  -rf   /"), "rm -rf /");
        assert_eq!(normalize_command("  \\rm  -rf  /  "), "rm -rf /");
        assert_eq!(
            normalize_command("\t\n rm \t\n -rf \t\n / \t\n"),
            "rm -rf /"
        );
    }

    #[test]
    fn test_normalize_preserves_safe_commands() {
        // Normalization should not make safe commands become dangerous
        let detector = DestructiveDetector::new(&config_with_preset(SafetyPreset::Moderate));
        assert!(!detector.is_destructive("  echo  hello  world  "));
        assert!(!detector.is_destructive("\\ls -la"));
        assert!(!detector.is_destructive("  git   status  "));
    }
}
