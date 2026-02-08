/// Parse a bash script file, extracting executable command lines.
///
/// Skips:
/// - Lines starting with `#` (comments, shebangs)
/// - Blank/whitespace-only lines
///
/// Each remaining line is trimmed and returned as one command.
#[must_use]
pub fn parse_bash_script(content: &str) -> Vec<String> {
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
    fn test_basic_script() {
        let content = "#!/bin/bash\n# comment\nls -la\n\ngit status";
        let result = parse_bash_script(content);
        assert_eq!(result, vec!["ls -la", "git status"]);
    }

    #[test]
    fn test_only_comments_and_blanks() {
        let content = "#!/bin/bash\n# just a comment\n\n# another\n";
        let result = parse_bash_script(content);
        assert!(result.is_empty());
    }

    #[test]
    fn test_trims_whitespace() {
        let content = "  echo hello  \n\tls  ";
        let result = parse_bash_script(content);
        assert_eq!(result, vec!["echo hello", "ls"]);
    }

    #[test]
    fn test_shebang_variants_skipped() {
        let content = "#!/usr/bin/env bash\necho hi";
        let result = parse_bash_script(content);
        assert_eq!(result, vec!["echo hi"]);
    }

    #[test]
    fn test_empty_content() {
        let result = parse_bash_script("");
        assert!(result.is_empty());
    }
}
