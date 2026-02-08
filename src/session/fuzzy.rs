//! Fuzzy matching for session name suggestions.
//!
//! Provides Levenshtein distance-based fuzzy matching, substring matching,
//! and UUID prefix matching to suggest similar session names when a user
//! mistypes a session identifier.

use strsim::levenshtein;

/// A suggestion for a similar session name with its edit distance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The suggested session name.
    pub name: String,
    /// The Levenshtein edit distance from the query (0 for substring/UUID matches).
    pub distance: usize,
}

/// Compute the scaled Levenshtein threshold based on **query** length.
///
/// - `query.len() <= 3`: threshold 1
/// - `query.len() <= 6`: threshold 2
/// - `query.len() >  6`: `max(3, query.len() / 3)`
fn threshold_for(query_len: usize) -> usize {
    if query_len <= 3 {
        1
    } else if query_len <= 6 {
        2
    } else {
        std::cmp::max(3, query_len / 3)
    }
}

/// Suggest sessions similar to the given query.
///
/// Searches through `all_names` (a slice of `(session_name, uuid)` tuples)
/// using three strategies:
/// 1. Levenshtein distance (case-insensitive) with scaled threshold
/// 2. Substring containment (case-insensitive)
/// 3. UUID prefix match (case-sensitive)
///
/// Results are sorted by distance ascending, then alphabetically for ties,
/// and truncated to `max_suggestions`.
#[must_use]
pub fn suggest_sessions(
    query: &str,
    all_names: &[(String, String)],
    max_suggestions: usize,
) -> Vec<Suggestion> {
    let query_lower = query.to_lowercase();
    let max_dist = threshold_for(query.len());

    let mut suggestions: Vec<Suggestion> = Vec::new();

    for (name, uuid) in all_names {
        let name_lower = name.to_lowercase();

        // Strategy 1: Levenshtein distance (case-insensitive)
        let dist = levenshtein(&query_lower, &name_lower);
        let lev_match = dist <= max_dist;

        // Strategy 2: Substring containment (case-insensitive)
        let substr_match = name_lower.contains(&query_lower);

        // Strategy 3: UUID prefix match (case-sensitive)
        let uuid_match = uuid.starts_with(query);

        if lev_match || substr_match || uuid_match {
            // Pick the best (lowest) distance for this candidate.
            // Substring and UUID matches get distance 0 to sort first.
            let effective_dist = if substr_match || uuid_match { 0 } else { dist };

            // Deduplicate: only add if not already present
            if !suggestions.iter().any(|s| s.name == *name) {
                suggestions.push(Suggestion {
                    name: name.clone(),
                    distance: effective_dist,
                });
            }
        }
    }

    // Sort by distance ascending, then alphabetically for ties
    suggestions.sort_by(|a, b| a.distance.cmp(&b.distance).then(a.name.cmp(&b.name)));

    suggestions.truncate(max_suggestions);
    suggestions
}

/// Format suggestions into a user-friendly help message.
///
/// - 0 suggestions: "help: No similar names found. List all sessions with: rec list"
/// - 1 suggestion: "help: Did you mean 'name'?"
/// - 2 suggestions: "help: Did you mean 'name1' or 'name2'?"
/// - 3+ suggestions: "help: Did you mean 'name1', 'name2', or 'name3'?"
///
/// # Panics
///
/// Panics if the suggestions slice is unexpectedly empty in the 3+ branch
/// (cannot happen due to the match guard).
#[must_use]
pub fn format_suggestions(suggestions: &[Suggestion]) -> String {
    match suggestions.len() {
        0 => "help: No similar names found. List all sessions with: rec list".to_string(),
        1 => format!("help: Did you mean '{}'?", suggestions[0].name),
        2 => format!(
            "help: Did you mean '{}' or '{}'?",
            suggestions[0].name, suggestions[1].name
        ),
        _ => {
            let all_but_last: Vec<String> = suggestions[..suggestions.len() - 1]
                .iter()
                .map(|s| format!("'{}'", s.name))
                .collect();
            format!(
                "help: Did you mean {}, or '{}'?",
                all_but_last.join(", "),
                suggestions.last().unwrap().name
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper ──────────────────────────────────────────────────────
    fn names(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(n, u)| (n.to_string(), u.to_string()))
            .collect()
    }

    // ── suggest_sessions tests ──────────────────────────────────────

    #[test]
    fn levenshtein_finds_close_match() {
        let all = names(&[("deploy-v1", "aaa")]);
        let results = suggest_sessions("deply-v1", &all, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "deploy-v1");
        assert_eq!(results[0].distance, 1);
    }

    #[test]
    fn substring_match_finds_containing_names() {
        let all = names(&[("deploy-v1", "aaa"), ("deploy-v2", "bbb")]);
        let results = suggest_sessions("ploy", &all, 5);
        assert_eq!(results.len(), 2);
        // Both should be found via substring
        let found_names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
        assert!(found_names.contains(&"deploy-v1"));
        assert!(found_names.contains(&"deploy-v2"));
    }

    #[test]
    fn uuid_prefix_match() {
        let all = names(&[("session-1", "a3f7b2c4-1234-5678-9abc-def012345678")]);
        let results = suggest_sessions("a3f", &all, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "session-1");
    }

    #[test]
    fn short_name_threshold_rejects_distant_matches() {
        // query "a" (len 1, threshold 1): "ab" distance 1 ✓, "xyz" distance 3 ✗, "abc" distance 2 ✗
        let all = names(&[("ab", "aaa"), ("xyz", "bbb"), ("abc", "ccc")]);
        let results = suggest_sessions("a", &all, 5);
        let found_names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
        assert!(
            found_names.contains(&"ab"),
            "should match 'ab' (distance 1)"
        );
        assert!(
            found_names.contains(&"abc"),
            "should match 'abc' (substring)"
        );
        assert!(
            !found_names.contains(&"xyz"),
            "should NOT match 'xyz' (distance 3)"
        );
    }

    #[test]
    fn max_suggestions_limits_results() {
        let all = names(&[
            ("deploy-v1", "aaa"),
            ("deploy-v2", "bbb"),
            ("deploy-latest", "ccc"),
            ("unrelated", "ddd"),
        ]);
        let results = suggest_sessions("deploy", &all, 3);
        assert!(results.len() <= 3);
        // "unrelated" should NOT be in the results (not a match)
        let found_names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
        assert!(!found_names.contains(&"unrelated"));
    }

    #[test]
    fn sorted_by_distance_then_alphabetically() {
        let all = names(&[
            ("deploy-v2", "aaa"),
            ("deploy-v1", "bbb"),
            ("deploy-latest", "ccc"),
        ]);
        let results = suggest_sessions("deploy", &all, 5);
        assert!(results.len() >= 2);
        // All are substring matches (distance 0), so sorted alphabetically
        if results.len() >= 2 {
            for i in 0..results.len() - 1 {
                if results[i].distance == results[i + 1].distance {
                    assert!(
                        results[i].name <= results[i + 1].name,
                        "Expected '{}' <= '{}' at same distance",
                        results[i].name,
                        results[i + 1].name
                    );
                } else {
                    assert!(
                        results[i].distance <= results[i + 1].distance,
                        "Expected distance {} <= {}",
                        results[i].distance,
                        results[i + 1].distance
                    );
                }
            }
        }
    }

    #[test]
    fn no_matches_returns_empty() {
        let all = names(&[("deploy-v1", "aaa")]);
        let results = suggest_sessions("zzzzzzz", &all, 5);
        assert!(results.is_empty());
    }

    #[test]
    fn case_insensitive_levenshtein() {
        let all = names(&[("Deploy-V1", "aaa")]);
        let results = suggest_sessions("deploy-v1", &all, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Deploy-V1");
    }

    #[test]
    fn case_insensitive_substring() {
        let all = names(&[("MyDeploy", "aaa")]);
        let results = suggest_sessions("mydep", &all, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "MyDeploy");
    }

    #[test]
    fn uuid_prefix_is_case_sensitive() {
        let all = names(&[("session-1", "a3F7b2c4-xxxx")]);
        // "a3f" should NOT match "a3F..." (case-sensitive UUID)
        let results = suggest_sessions("a3f", &all, 5);
        // It may match via substring of the name though — "a3f" is not in "session-1"
        // and UUID prefix is case-sensitive, so no match expected
        assert!(
            results.is_empty(),
            "UUID prefix should be case-sensitive: {results:?}"
        );
    }

    #[test]
    fn medium_name_threshold() {
        // query "test" (len 4, threshold 2): "tset" distance 2 ✓, "abcd" distance 4 ✗
        let all = names(&[("tset", "aaa"), ("abcd", "bbb")]);
        let results = suggest_sessions("test", &all, 5);
        let found_names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
        assert!(
            found_names.contains(&"tset"),
            "should match 'tset' (distance 2)"
        );
        assert!(
            !found_names.contains(&"abcd"),
            "should NOT match 'abcd' (distance 4)"
        );
    }

    #[test]
    fn long_name_threshold() {
        // query "deployment-v1" (len 13, threshold max(3, 13/3)=4):
        // "deployment-v2" distance 1 ✓, "xxxxxxxxxxx" distance large ✗
        let all = names(&[("deployment-v2", "aaa"), ("xxxxxxxxxxx", "bbb")]);
        let results = suggest_sessions("deployment-v1", &all, 5);
        let found_names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
        assert!(
            found_names.contains(&"deployment-v2"),
            "should match 'deployment-v2'"
        );
        assert!(
            !found_names.contains(&"xxxxxxxxxxx"),
            "should NOT match 'xxxxxxxxxxx'"
        );
    }

    #[test]
    fn deduplicates_matches() {
        // A name that matches via both Levenshtein AND substring should appear only once
        let all = names(&[("deploy", "aaa")]);
        let results = suggest_sessions("deploy", &all, 5);
        assert_eq!(results.len(), 1, "should not have duplicates");
    }

    // ── format_suggestions tests ────────────────────────────────────

    #[test]
    fn format_zero_suggestions() {
        let result = format_suggestions(&[]);
        assert_eq!(
            result,
            "help: No similar names found. List all sessions with: rec list"
        );
    }

    #[test]
    fn format_one_suggestion() {
        let suggestions = vec![Suggestion {
            name: "deploy-v1".to_string(),
            distance: 1,
        }];
        assert_eq!(
            format_suggestions(&suggestions),
            "help: Did you mean 'deploy-v1'?"
        );
    }

    #[test]
    fn format_two_suggestions() {
        let suggestions = vec![
            Suggestion {
                name: "deploy-v1".to_string(),
                distance: 1,
            },
            Suggestion {
                name: "deploy-v2".to_string(),
                distance: 2,
            },
        ];
        assert_eq!(
            format_suggestions(&suggestions),
            "help: Did you mean 'deploy-v1' or 'deploy-v2'?"
        );
    }

    #[test]
    fn format_three_suggestions() {
        let suggestions = vec![
            Suggestion {
                name: "deploy-v1".to_string(),
                distance: 1,
            },
            Suggestion {
                name: "deploy-v2".to_string(),
                distance: 2,
            },
            Suggestion {
                name: "deploy-latest".to_string(),
                distance: 3,
            },
        ];
        assert_eq!(
            format_suggestions(&suggestions),
            "help: Did you mean 'deploy-v1', 'deploy-v2', or 'deploy-latest'?"
        );
    }
}
