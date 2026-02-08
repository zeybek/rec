/// Parse a bash history file, extracting command lines.
///
/// Skips:
/// - Lines starting with `#` (timestamps from HISTTIMEFORMAT)
/// - Blank/whitespace-only lines
///
/// Each remaining line is trimmed and returned as one command.
#[must_use]
pub fn parse_bash_history(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(std::string::ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_history() {
        let content = "ls -la\ngit status\ndocker compose up -d";
        let result = parse_bash_history(content);
        assert_eq!(result, vec!["ls -la", "git status", "docker compose up -d"]);
    }

    #[test]
    fn test_history_with_timestamps() {
        let content = "ls -la\n#1625000000\ngit status\n";
        let result = parse_bash_history(content);
        assert_eq!(result, vec!["ls -la", "git status"]);
    }

    #[test]
    fn test_skips_blank_lines() {
        let content = "echo hello\n\n\nls\n";
        let result = parse_bash_history(content);
        assert_eq!(result, vec!["echo hello", "ls"]);
    }

    #[test]
    fn test_trims_whitespace() {
        let content = "  echo hello  \n  ls -la  ";
        let result = parse_bash_history(content);
        assert_eq!(result, vec!["echo hello", "ls -la"]);
    }

    #[test]
    fn test_empty_content() {
        let result = parse_bash_history("");
        assert!(result.is_empty());
    }
}
