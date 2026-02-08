/// Parse a fish history file (YAML-like format, NOT valid YAML).
///
/// Fish history entries start with `- cmd: COMMAND` followed by optional
/// `  when: TIMESTAMP` and `  paths:` lines.
///
/// Escaping:
/// - `\n` in command → literal newline
/// - `\\` in command → literal backslash
#[must_use]
pub fn parse_fish_history(content: &str) -> Vec<String> {
    let mut commands = Vec::new();

    for line in content.lines() {
        if let Some(cmd) = line.strip_prefix("- cmd: ") {
            commands.push(fish_unescape(cmd));
        }
        // Ignore `when:`, `paths:`, path entries, and other lines
    }

    commands
}

/// Unescape fish history escape sequences.
///
/// Fish uses `\n` for literal newline and `\\` for literal backslash.
/// Character-by-character scan avoids sentinel-based workarounds.
fn fish_unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('\\') | None => result.push('\\'),
                Some(other) => {
                    // Unknown escape — preserve literally
                    result.push('\\');
                    result.push(other);
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_entries() {
        let content = "- cmd: ls -la\n  when: 123\n- cmd: echo hi";
        let result = parse_fish_history(content);
        assert_eq!(result, vec!["ls -la", "echo hi"]);
    }

    #[test]
    fn test_unescape_newline() {
        let content = "- cmd: echo line1\\nline2";
        let result = parse_fish_history(content);
        assert_eq!(result, vec!["echo line1\nline2"]);
    }

    #[test]
    fn test_unescape_backslash() {
        let content = "- cmd: echo path\\\\to\\\\file";
        let result = parse_fish_history(content);
        assert_eq!(result, vec!["echo path\\to\\file"]);
    }

    #[test]
    fn test_ignores_when_and_paths() {
        let content = "\
- cmd: git checkout file.txt
  when: 1565133290
  paths:
    - file.txt
- cmd: ls";
        let result = parse_fish_history(content);
        assert_eq!(result, vec!["git checkout file.txt", "ls"]);
    }

    #[test]
    fn test_empty_content() {
        let result = parse_fish_history("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_mixed_escapes() {
        // Both \n and \\ in the same command
        let content = "- cmd: echo \\\\hello\\nworld";
        let result = parse_fish_history(content);
        assert_eq!(result, vec!["echo \\hello\nworld"]);
    }
}
