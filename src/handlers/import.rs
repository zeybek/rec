use super::HandlerContext;
use super::common::{handle_result, print_json};
use rec::import::import_file;
use rec::storage::SessionStore;
use std::path::Path;
use std::process::ExitCode;

/// Handle the `import` command — import commands from a file into a new session.
///
/// Supports importing from shell history files (bash, zsh, fish) and script files.
/// Auto-detects the format based on file content.
///
/// # Errors
/// Returns exit code 1 on user errors (file not found, unrecognized format).
/// Returns exit code 2 on I/O failures.
pub fn handle_import(ctx: &HandlerContext, file: &Path, name: Option<&String>) -> ExitCode {
    let store = SessionStore::new(ctx.paths.clone());

    let result = match import_file(file, name.map(String::as_str), &store) {
        Ok(result) => {
            if ctx.output.json {
                let json = serde_json::json!({
                    "session": result.session_name,
                    "commands": result.command_count,
                    "format": result.format.to_string(),
                    "preview": result.preview_commands,
                });
                print_json(&json);
            } else {
                ctx.output.success(&format!(
                    "Imported {} commands as '{}'",
                    result.command_count, result.session_name
                ));
                ctx.output.info(&format!("Format: {}", result.format));
                if !result.preview_commands.is_empty() {
                    ctx.output.info("Preview:");
                    for (i, cmd) in result.preview_commands.iter().enumerate() {
                        println!("  {}. {}", i + 1, cmd);
                    }
                    if result.command_count > result.preview_commands.len() {
                        println!(
                            "  ... and {} more",
                            result.command_count - result.preview_commands.len()
                        );
                    }
                }
            }
            Ok(())
        }
        Err(e) => Err(e),
    };
    handle_result(result, ctx.output, &ctx.paths)
}
