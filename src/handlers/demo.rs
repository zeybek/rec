use super::HandlerContext;
use super::common::print_json;
use rec::error::EXIT_USER_ERROR;
use std::process::ExitCode;

/// Handle the `demo` command — runs an interactive walkthrough.
pub fn handle_demo(_ctx: &HandlerContext) -> ExitCode {
    use rec::demo;
    use std::io::IsTerminal;

    let is_tty = std::io::stdout().is_terminal();
    demo::run_demo(is_tty);
    ExitCode::SUCCESS
}

/// Handle the `doctor` command — diagnoses installation issues.
pub fn handle_doctor(ctx: &HandlerContext) -> ExitCode {
    use rec::doctor;

    let results = doctor::run_all_checks();
    let has_failure = results
        .iter()
        .any(|r| matches!(r.status, doctor::CheckStatus::Fail));

    if ctx.output.json {
        let json = doctor::format_report_json(&results);
        print_json(&json);
    } else {
        let report = doctor::format_report(&results, ctx.output.use_color());
        print!("{report}");
    }

    if has_failure {
        return ExitCode::from(EXIT_USER_ERROR);
    }
    ExitCode::SUCCESS
}
