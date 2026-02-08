//! Diagnostic checks for installation health.
//!
//! Runs a suite of checks (shell hooks, config, storage, etc.) and produces
//! a report with pass/fail/warn status and fix hints.

pub mod checks;
pub mod report;

pub use checks::*;
pub use report::*;

/// Run all diagnostic checks and return a vector of results.
#[must_use]
pub fn run_all_checks() -> Vec<CheckResult> {
    vec![
        check_rec_version(),
        check_rec_in_path(),
        check_shell_detected(),
        check_shell_hooks_installed(),
        check_config_valid(),
        check_storage_dir_exists(),
        check_storage_writable(),
        check_rc_file_writable(),
        check_data_dir_permissions(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_rec_version_always_passes() {
        let result = check_rec_version();
        assert_eq!(result.status, CheckStatus::Pass);
        assert_eq!(result.name, "rec version");
        assert!(!result.message.is_empty());
        assert!(result.fix_hint.is_none());
    }

    #[test]
    fn check_rec_in_path_returns_result() {
        let result = check_rec_in_path();
        assert_eq!(result.name, "rec in PATH");
    }

    #[test]
    fn check_shell_detected_returns_result() {
        let result = check_shell_detected();
        assert_eq!(result.name, "Shell detected");
    }

    #[test]
    fn check_config_valid_returns_result() {
        let result = check_config_valid();
        assert_eq!(result.name, "Config file");
        assert!(
            result.status == CheckStatus::Pass,
            "Expected pass, got: {:?} — {}",
            result.status,
            result.message
        );
    }

    #[test]
    fn run_all_checks_returns_nine_results() {
        let results = run_all_checks();
        assert_eq!(results.len(), 9, "Expected 9 checks, got {}", results.len());
    }

    #[test]
    fn format_report_contains_header_and_summary() {
        let results = vec![
            CheckResult {
                name: "test check",
                status: CheckStatus::Pass,
                message: "ok".to_string(),
                fix_hint: None,
            },
            CheckResult {
                name: "warn check",
                status: CheckStatus::Warn,
                message: "maybe".to_string(),
                fix_hint: Some("try this".to_string()),
            },
            CheckResult {
                name: "fail check",
                status: CheckStatus::Fail,
                message: "bad".to_string(),
                fix_hint: Some("fix it".to_string()),
            },
        ];

        let report = format_report(&results, false);
        assert!(report.starts_with("rec doctor"), "Should start with header");
        assert!(
            report.contains("test check: ok"),
            "Should contain pass check"
        );
        assert!(
            report.contains("warn check: maybe"),
            "Should contain warn check"
        );
        assert!(
            report.contains("fail check: bad"),
            "Should contain fail check"
        );
        assert!(report.contains("Fix: try this"), "Should contain fix hint");
        assert!(report.contains("Fix: fix it"), "Should contain fix hint");
        assert!(
            report.contains("1 passed, 1 warnings, 1 failed"),
            "Should contain summary: got {report}"
        );
    }

    #[test]
    fn format_report_colored_has_ansi_escapes() {
        let results = vec![CheckResult {
            name: "color test",
            status: CheckStatus::Pass,
            message: "ok".to_string(),
            fix_hint: None,
        }];

        let report = format_report(&results, true);
        assert!(
            report.contains("\x1b[32m"),
            "Colored report should contain green ANSI code"
        );
    }

    #[test]
    fn format_report_json_structure() {
        let results = vec![
            CheckResult {
                name: "a",
                status: CheckStatus::Pass,
                message: "ok".to_string(),
                fix_hint: None,
            },
            CheckResult {
                name: "b",
                status: CheckStatus::Fail,
                message: "bad".to_string(),
                fix_hint: Some("fix".to_string()),
            },
        ];

        let json_val = format_report_json(&results);
        let checks = json_val["checks"].as_array().unwrap();
        assert_eq!(checks.len(), 2);

        assert_eq!(checks[0]["name"], "a");
        assert_eq!(checks[0]["status"], "pass");
        assert!(checks[0]["fix_hint"].is_null());

        assert_eq!(checks[1]["name"], "b");
        assert_eq!(checks[1]["status"], "fail");
        assert_eq!(checks[1]["fix_hint"], "fix");

        assert_eq!(json_val["summary"]["pass"], 1);
        assert_eq!(json_val["summary"]["warn"], 0);
        assert_eq!(json_val["summary"]["fail"], 1);
    }

    #[test]
    fn check_status_as_str() {
        assert_eq!(CheckStatus::Pass.as_str(), "pass");
        assert_eq!(CheckStatus::Warn.as_str(), "warn");
        assert_eq!(CheckStatus::Fail.as_str(), "fail");
    }

    #[test]
    fn fail_results_always_have_fix_hints() {
        let results = run_all_checks();
        for result in &results {
            if result.status == CheckStatus::Fail {
                assert!(
                    result.fix_hint.is_some(),
                    "Fail result '{}' should have a fix_hint",
                    result.name
                );
            }
        }
    }
}
