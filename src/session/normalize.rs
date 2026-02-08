//! Tag and alias normalization and validation pipeline.
//!
//! Normalizes tags to lowercase-hyphenated form and detects collisions
//! between normalized variants of existing tags. Also provides validation
//! functions for alias and tag names.

use crate::error::RecError;

/// Normalize a tag to lowercase-hyphenated form.
///
/// - Trims leading/trailing whitespace
/// - Collapses internal whitespace to single hyphens
/// - Lowercases all characters
///
/// # Examples
/// ```
/// use rec::session::normalize::normalize_tag;
/// assert_eq!(normalize_tag("  My  Deploy  Tag  "), "my-deploy-tag");
/// ```
#[must_use]
pub fn normalize_tag(tag: &str) -> String {
    tag.split_whitespace()
        .collect::<Vec<&str>>()
        .join("-")
        .to_lowercase()
}

/// Validate an alias name.
///
/// Alias names must be non-empty and contain only alphanumeric characters,
/// dashes, and underscores. This prevents filesystem issues and injection attacks.
///
/// # Errors
///
/// Returns `RecError::InvalidAliasName` if the name is empty or contains
/// characters other than alphanumeric, dash, or underscore.
///
/// # Examples
/// ```
/// use rec::session::normalize::validate_alias_name;
/// assert!(validate_alias_name("my-alias").is_ok());
/// assert!(validate_alias_name("alias_123").is_ok());
/// assert!(validate_alias_name("").is_err());
/// assert!(validate_alias_name("bad/alias").is_err());
/// ```
pub fn validate_alias_name(name: &str) -> crate::error::Result<()> {
    if name.is_empty() {
        return Err(RecError::InvalidAliasName(
            "alias name cannot be empty".to_string(),
        ));
    }
    if name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err(RecError::InvalidAliasName(name.to_string()))
    }
}

/// Validate a tag name after normalization.
///
/// Tag names must be non-empty and contain only alphanumeric characters,
/// dashes, and underscores after normalization. This prevents injection attacks
/// and ensures consistent tag storage.
///
/// # Errors
///
/// Returns `RecError::InvalidTagName` if the normalized tag is empty or contains
/// characters other than alphanumeric, dash, or underscore.
///
/// # Examples
/// ```
/// use rec::session::normalize::validate_tag_name;
/// assert!(validate_tag_name("deploy").is_ok());
/// assert!(validate_tag_name("ci-cd").is_ok());
/// assert!(validate_tag_name("tag_123").is_ok());
/// assert!(validate_tag_name("").is_err());
/// assert!(validate_tag_name("bad@tag").is_err());
/// ```
pub fn validate_tag_name(tag: &str) -> crate::error::Result<()> {
    if tag.is_empty() {
        return Err(RecError::InvalidTagName(
            "tag name cannot be empty".to_string(),
        ));
    }
    if tag
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err(RecError::InvalidTagName(tag.to_string()))
    }
}

/// Detect when a normalized tag collides with an existing tag.
///
/// A collision occurs when an existing tag normalizes to the same form as
/// `normalized`, but the original string differs from `normalized` (i.e.,
/// not an exact match).
///
/// Returns the original (un-normalized) form of the first colliding tag,
/// or `None` if no collision.
///
/// # Examples
/// ```
/// use rec::session::normalize::find_tag_collision;
/// let existing = vec!["Deploy".to_string(), "rust".to_string()];
/// assert_eq!(find_tag_collision("deploy", &existing), Some("Deploy".to_string()));
/// ```
#[must_use]
pub fn find_tag_collision(normalized: &str, existing_tags: &[String]) -> Option<String> {
    existing_tags.iter().find_map(|existing| {
        let existing_normalized = normalize_tag(existing);
        if existing_normalized == normalized && existing != normalized {
            Some(existing.clone())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_tag tests ──────────────────────────────────────────

    #[test]
    fn normalize_tag_lowercases() {
        assert_eq!(normalize_tag("Deploy"), "deploy");
    }

    #[test]
    fn normalize_tag_trims_and_collapses_whitespace() {
        assert_eq!(normalize_tag("  My  Deploy  Tag  "), "my-deploy-tag");
    }

    #[test]
    fn normalize_tag_already_normalized() {
        assert_eq!(normalize_tag("rust"), "rust");
    }

    #[test]
    fn normalize_tag_empty_after_trim() {
        assert_eq!(normalize_tag("  "), "");
    }

    #[test]
    fn normalize_tag_preserves_hyphens_and_lowercases() {
        assert_eq!(normalize_tag("CI-CD"), "ci-cd");
    }

    #[test]
    fn normalize_tag_trims_only() {
        assert_eq!(normalize_tag("  HELLO  "), "hello");
    }

    #[test]
    fn normalize_tag_no_change_when_already_hyphenated() {
        assert_eq!(normalize_tag("my-tag"), "my-tag");
    }

    // ── find_tag_collision tests ─────────────────────────────────────

    #[test]
    fn collision_detected_case_variant() {
        let existing = vec!["Deploy".to_string(), "rust".to_string()];
        assert_eq!(
            find_tag_collision("deploy", &existing),
            Some("Deploy".to_string())
        );
    }

    #[test]
    fn no_collision_on_exact_match() {
        let existing = vec!["deploy".to_string(), "rust".to_string()];
        assert_eq!(find_tag_collision("deploy", &existing), None);
    }

    #[test]
    fn collision_detected_whitespace_variant() {
        let existing = vec!["My Deploy Tag".to_string()];
        assert_eq!(
            find_tag_collision("my-deploy-tag", &existing),
            Some("My Deploy Tag".to_string())
        );
    }

    #[test]
    fn no_collision_when_tag_absent() {
        let existing = vec!["deploy".to_string(), "rust".to_string()];
        assert_eq!(find_tag_collision("new-tag", &existing), None);
    }

    #[test]
    fn no_collision_with_empty_list() {
        let existing: Vec<String> = vec![];
        assert_eq!(find_tag_collision("deploy", &existing), None);
    }

    // ── validate_alias_name tests ─────────────────────────────────────

    #[test]
    fn validate_alias_name_valid_alphanumeric() {
        assert!(validate_alias_name("deploy").is_ok());
        assert!(validate_alias_name("myAlias123").is_ok());
        assert!(validate_alias_name("a").is_ok());
    }

    #[test]
    fn validate_alias_name_valid_with_dash() {
        assert!(validate_alias_name("my-alias").is_ok());
        assert!(validate_alias_name("deploy-prod").is_ok());
        assert!(validate_alias_name("ci-cd-2026").is_ok());
    }

    #[test]
    fn validate_alias_name_valid_with_underscore() {
        assert!(validate_alias_name("my_alias").is_ok());
        assert!(validate_alias_name("deploy_prod").is_ok());
        assert!(validate_alias_name("ci_cd_2026").is_ok());
    }

    #[test]
    fn validate_alias_name_valid_mixed() {
        assert!(validate_alias_name("my-alias_123").is_ok());
        assert!(validate_alias_name("Deploy_Prod-v2").is_ok());
    }

    #[test]
    fn validate_alias_name_empty() {
        let result = validate_alias_name("");
        assert!(result.is_err());
        match result.unwrap_err() {
            RecError::InvalidAliasName(msg) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected InvalidAliasName error"),
        }
    }

    #[test]
    fn validate_alias_name_invalid_space() {
        let result = validate_alias_name("my alias");
        assert!(result.is_err());
        match result.unwrap_err() {
            RecError::InvalidAliasName(name) => {
                assert_eq!(name, "my alias");
            }
            _ => panic!("Expected InvalidAliasName error"),
        }
    }

    #[test]
    fn validate_alias_name_invalid_special_chars() {
        assert!(validate_alias_name("bad/alias").is_err());
        assert!(validate_alias_name("alias@home").is_err());
        assert!(validate_alias_name("alias.ext").is_err());
        assert!(validate_alias_name("alias$var").is_err());
        assert!(validate_alias_name("alias;cmd").is_err());
        assert!(validate_alias_name("alias|pipe").is_err());
        assert!(validate_alias_name("alias&bg").is_err());
        assert!(validate_alias_name("alias<redirect").is_err());
        assert!(validate_alias_name("alias>redirect").is_err());
    }

    // ── validate_tag_name tests ───────────────────────────────────────

    #[test]
    fn validate_tag_name_valid_alphanumeric() {
        assert!(validate_tag_name("deploy").is_ok());
        assert!(validate_tag_name("rust").is_ok());
        assert!(validate_tag_name("v1").is_ok());
    }

    #[test]
    fn validate_tag_name_valid_with_dash() {
        assert!(validate_tag_name("ci-cd").is_ok());
        assert!(validate_tag_name("my-tag").is_ok());
        assert!(validate_tag_name("deploy-2026").is_ok());
    }

    #[test]
    fn validate_tag_name_valid_with_underscore() {
        assert!(validate_tag_name("ci_cd").is_ok());
        assert!(validate_tag_name("my_tag").is_ok());
        assert!(validate_tag_name("deploy_2026").is_ok());
    }

    #[test]
    fn validate_tag_name_valid_mixed() {
        assert!(validate_tag_name("ci-cd_2026").is_ok());
        assert!(validate_tag_name("my_tag-v2").is_ok());
    }

    #[test]
    fn validate_tag_name_empty() {
        let result = validate_tag_name("");
        assert!(result.is_err());
        match result.unwrap_err() {
            RecError::InvalidTagName(msg) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected InvalidTagName error"),
        }
    }

    #[test]
    fn validate_tag_name_invalid_special_chars() {
        assert!(validate_tag_name("bad/tag").is_err());
        assert!(validate_tag_name("tag@home").is_err());
        assert!(validate_tag_name("tag.ext").is_err());
        assert!(validate_tag_name("tag$var").is_err());
        assert!(validate_tag_name("tag;cmd").is_err());
        assert!(validate_tag_name("tag|pipe").is_err());
        assert!(validate_tag_name("tag&bg").is_err());
    }
}
