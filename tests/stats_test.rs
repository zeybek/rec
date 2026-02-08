//! Integration tests for `rec stats` command (DATA-08, DATA-09).

mod common;

use common::{TestEnv, normal_output};
use rec::session::{compute_stats, format_stats};

/// DATA-08: Stats shows session count, command count, and duration.
#[test]
fn test_stats_shows_command_count_and_duration() {
    let env = TestEnv::new();
    env.create_and_save_session("session-1");
    env.create_and_save_session("session-2");

    let stats = compute_stats(&env.store).unwrap();

    assert_eq!(stats.total_sessions, 2);
    assert_eq!(stats.total_commands, 2); // 1 command per session
    assert!(stats.total_duration_secs >= 0.0);
}

/// DATA-09: Stats JSON output produces valid result.
#[test]
fn test_stats_json_produces_valid_output() {
    let env = TestEnv::new();
    env.create_and_save_session("session-1");

    let stats = compute_stats(&env.store).unwrap();
    let output = normal_output();
    let result = format_stats(&stats, true, &output);

    assert!(result.is_ok());
}

/// Edge case: empty storage returns zero stats.
#[test]
fn test_stats_empty_storage() {
    let env = TestEnv::new();

    let stats = compute_stats(&env.store).unwrap();

    assert_eq!(stats.total_sessions, 0);
    assert_eq!(stats.total_commands, 0);
    #[allow(clippy::float_cmp)]
    {
        assert_eq!(stats.total_duration_secs, 0.0);
    }
    assert_eq!(stats.storage_bytes, 0);
    assert!(stats.tag_counts.is_empty());
}
