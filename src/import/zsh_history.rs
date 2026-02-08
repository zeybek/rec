/// Parse a zsh history file, supporting both extended and plain formats.
///
/// Extended format: `: TIMESTAMP:DURATION;COMMAND`
/// - Extracts the command portion after the semicolon
///
/// Plain format: one command per line (like bash history)
///
/// Skips blank lines.
///
/// # Panics
///
/// Panics if the internal regex pattern is invalid (should never happen).
pub fn parse_zsh_history(content: &str) -> Vec<String> {
    use std::sync::OnceLock;
    static EXTENDED_RE: OnceLock<regex::Regex> = OnceLock::new();
    let extended_re = EXTENDED_RE.get_or_init(|| regex::Regex::new(r"^: \d+:\d+;(.*)$").unwrap());

    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            // Try extended format first
            if let Some(caps) = extended_re.captures(trimmed) {
                let cmd = caps.get(1).unwrap().as_str().to_string();
                if cmd.is_empty() { None } else { Some(cmd) }
            } else {
                // Plain format: treat as regular line
                Some(trimmed.to_string())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extended_format() {
        let content = ": 1458291931:0;ls -l\n: 1458291945:3;git push";
        let result = parse_zsh_history(content);
        assert_eq!(result, vec!["ls -l", "git push"]);
    }

    #[test]
    fn test_plain_format() {
        let content = "ls -l\ngit push";
        let result = parse_zsh_history(content);
        assert_eq!(result, vec!["ls -l", "git push"]);
    }

    #[test]
    fn test_mixed_format() {
        // Extended lines mixed with plain lines
        let content = ": 1458291931:0;ls -l\nplain command";
        let result = parse_zsh_history(content);
        assert_eq!(result, vec!["ls -l", "plain command"]);
    }

    #[test]
    fn test_skips_blank_lines() {
        let content = ": 1458291931:0;ls -l\n\n: 1458291945:3;git push\n";
        let result = parse_zsh_history(content);
        assert_eq!(result, vec!["ls -l", "git push"]);
    }

    #[test]
    fn test_command_with_semicolons() {
        // Command itself contains semicolons — only first is format delimiter
        let content = ": 1458291931:0;echo a; echo b";
        let result = parse_zsh_history(content);
        assert_eq!(result, vec!["echo a; echo b"]);
    }

    #[test]
    fn test_zero_duration() {
        let content = ": 1458291960:0;docker compose up -d";
        let result = parse_zsh_history(content);
        assert_eq!(result, vec!["docker compose up -d"]);
    }

    #[test]
    fn test_empty_content() {
        let result = parse_zsh_history("");
        assert!(result.is_empty());
    }
}
