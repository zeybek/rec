use super::HandlerContext;
use super::common::handle_resolve_error;
use rec::cli::ExportFormat;
use rec::error::{EXIT_SYSTEM_ERROR, EXIT_USER_ERROR};
use rec::export::parameterize::{Parameter, detect_all_parameters};
use rec::session::resolve_session_with_alias;
use rec::storage::{AliasStore, SessionStore};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

/// Handle the `export` command — export a session to various formats.
///
/// Supports bash scripts, Makefiles, Markdown, GitHub Actions, GitLab CI,
/// Dockerfiles, and CircleCI. Optionally parameterizes detected values
/// (paths, ports, etc.) for reusable templates.
///
/// # Errors
/// Returns exit code 1 on user errors (session not found, invalid parameters).
/// Returns exit code 2 on I/O failures.
pub fn handle_export(
    ctx: &HandlerContext,
    identifier: &str,
    format: &ExportFormat,
    output_file: Option<&PathBuf>,
    parameterize: bool,
    param_args: &[String],
) -> ExitCode {
    let store = SessionStore::new(ctx.paths.clone());
    let alias_store = AliasStore::new(&ctx.paths);
    let interactive = rec::replay::prompt::is_interactive();
    let session = match resolve_session_with_alias(&store, &alias_store, identifier, interactive) {
        Ok(s) => s,
        Err(e) => return handle_resolve_error(&e, ctx.output),
    };

    // Parse --param KEY=VALUE args into a HashMap
    let mut param_overrides: HashMap<String, String> = HashMap::new();
    for arg in param_args {
        if let Some((key, value)) = arg.split_once('=') {
            param_overrides.insert(key.to_string(), value.to_string());
        } else {
            ctx.output.error(
                "Invalid parameter",
                &format!("'{arg}' is not a valid KEY=VALUE pair"),
                None,
                Some("Use format: --param KEY=VALUE"),
            );
            return ExitCode::from(EXIT_USER_ERROR);
        }
    }

    // Resolve parameters if --parameterize is set
    let resolved_params: Option<Vec<Parameter>> = if parameterize {
        let mut params = detect_all_parameters(&session);

        if params.is_empty() {
            eprintln!("No parameterizable values detected");
            None
        } else {
            // Print summary to stderr
            let names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
            eprintln!("Found {} parameters: {}", params.len(), names.join(", "));

            // First pass: apply all --param overrides
            for param in &mut params {
                if let Some(override_val) = param_overrides.get(&param.name) {
                    param.value = Some(override_val.clone());
                }
            }

            // Check if all params have values from --param overrides
            let all_params_provided = params.iter().all(|p| p.value.is_some());

            if all_params_provided {
                eprintln!("All parameters provided via --param, skipping interactive prompt");
            } else {
                // Second pass: prompt only for missing params
                for param in &mut params {
                    if param.value.is_some() {
                        continue;
                    }
                    if interactive {
                        let default_val = &param.original;
                        let prompted: String = dialoguer::Input::new()
                            .with_prompt(format!("{} [{}]", param.name, default_val))
                            .default(default_val.clone())
                            .interact_text()
                            .unwrap_or_else(|_| default_val.clone());
                        param.value = Some(prompted);
                    } else {
                        param.value = Some(param.original.clone());
                    }
                }
            }

            Some(params)
        }
    } else {
        None
    };

    let params_ref = resolved_params.as_deref();

    let content = match format {
        ExportFormat::Bash => rec::export::bash::export_bash(&session, params_ref),
        ExportFormat::Makefile => rec::export::makefile::export_makefile(&session, params_ref),
        ExportFormat::Markdown => rec::export::markdown::export_markdown(&session, params_ref),
        ExportFormat::GithubAction => {
            rec::export::github_action::export_github_action(&session, params_ref)
        }
        ExportFormat::GitlabCi => rec::export::gitlab_ci::export_gitlab_ci(&session, params_ref),
        ExportFormat::Dockerfile => {
            rec::export::dockerfile::export_dockerfile(&session, params_ref)
        }
        ExportFormat::Circleci => rec::export::circleci::export_circleci(&session, params_ref),
    };

    match output_file {
        Some(path) => {
            if let Err(e) = std::fs::write(path, &content) {
                ctx.output.error(
                    "Write failed",
                    &format!("Failed to write to {}: {}", path.display(), e),
                    None,
                    None,
                );
                return ExitCode::from(EXIT_SYSTEM_ERROR);
            }
            ctx.output
                .success(&format!("Exported to {}", path.display()));
        }
        None => {
            print!("{content}");
        }
    }

    ExitCode::SUCCESS
}
