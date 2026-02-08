//! Integration tests for doctor diagnostics (CONF-15 to CONF-17).

use rec::doctor::{self, CheckStatus};

/// CONF-15: Doctor runs all checks and returns a non-empty Vec of `CheckResult`.
#[test]
fn test_doctor_runs_all_checks() {
    let results = doctor::run_all_checks();

    assert!(
        !results.is_empty(),
        "Doctor should return at least one check"
    );
    assert_eq!(results.len(), 9, "Doctor should run exactly 9 checks");

    // Every result should have a name and non-empty message
    for result in &results {
        assert!(!result.name.is_empty(), "Check should have a name");
        assert!(!result.message.is_empty(), "Check should have a message");
        // Status must be one of the valid variants
        assert!(
            matches!(
                result.status,
                CheckStatus::Pass | CheckStatus::Warn | CheckStatus::Fail
            ),
            "Check '{}' has invalid status",
            result.name
        );
    }
}

/// CONF-16: `format_report` produces non-empty output containing check names.
#[test]
fn test_doctor_format_report_produces_output() {
    let results = doctor::run_all_checks();
    let report = doctor::format_report(&results, false);

    assert!(!report.is_empty(), "Report should not be empty");
    assert!(
        report.contains("rec doctor"),
        "Report should contain 'rec doctor' header"
    );
    assert!(
        report.contains("rec version"),
        "Report should contain 'rec version' check name"
    );
    assert!(
        report.contains("passed"),
        "Report should contain summary with 'passed'"
    );
}

/// CONF-17: `format_report_json` produces valid JSON with expected structure.
#[test]
fn test_doctor_json_output_is_valid() {
    let results = doctor::run_all_checks();
    let json_val = doctor::format_report_json(&results);

    // Verify it serializes to valid JSON string
    let json_str = serde_json::to_string(&json_val).expect("Should serialize to JSON string");
    assert!(!json_str.is_empty(), "JSON output should not be empty");

    // Verify structure: has "checks" array and "summary" object
    let checks = json_val["checks"]
        .as_array()
        .expect("Should have 'checks' array");
    assert_eq!(checks.len(), 9, "Should have 9 check results in JSON");

    // Each check should have name, status, message fields
    for check in checks {
        assert!(check["name"].is_string(), "Check should have string 'name'");
        assert!(
            check["status"].is_string(),
            "Check should have string 'status'"
        );
        let status = check["status"].as_str().unwrap();
        assert!(
            ["pass", "warn", "fail"].contains(&status),
            "Status should be pass/warn/fail, got: {status}"
        );
        assert!(
            check["message"].is_string(),
            "Check should have string 'message'"
        );
    }

    // Summary should have pass/warn/fail counts
    let summary = &json_val["summary"];
    assert!(
        summary["pass"].is_number(),
        "Summary should have 'pass' count"
    );
    assert!(
        summary["warn"].is_number(),
        "Summary should have 'warn' count"
    );
    assert!(
        summary["fail"].is_number(),
        "Summary should have 'fail' count"
    );
}
