use super::HandlerContext;
use super::common::{handle_error, print_json};
use rec::error::EXIT_USER_ERROR;
use rec::session::validate_alias_name;
use rec::storage::{AliasStore, SessionStore};
use std::process::ExitCode;

pub fn handle_alias(
    ctx: &HandlerContext,
    name: Option<&String>,
    session: Option<&String>,
    list: bool,
    remove: Option<&String>,
) -> ExitCode {
    let alias_store = AliasStore::new(&ctx.paths);

    // --list: show all aliases
    if list {
        let aliases = match alias_store.list() {
            Ok(a) => a,
            Err(e) => return handle_error(&e, ctx.output),
        };
        if aliases.is_empty() {
            if ctx.output.json {
                println!("[]");
            } else {
                ctx.output.info("No aliases defined");
                ctx.output.info("Create one with: rec alias NAME SESSION");
            }
        } else if ctx.output.json {
            let json_entries: Vec<serde_json::Value> = aliases
                .iter()
                .map(|(alias, target)| {
                    serde_json::json!({
                        "alias": alias,
                        "session": target
                    })
                })
                .collect();
            print_json(&serde_json::Value::Array(json_entries));
        } else {
            println!();
            let max_len = aliases.iter().map(|(a, _)| a.len()).max().unwrap_or(0);
            for (alias, target) in &aliases {
                println!("  {alias:<max_len$}  ->  {target}");
            }
            println!();
            ctx.output.info(&format!("{} alias(es)", aliases.len()));
        }
        return ExitCode::SUCCESS;
    }

    // --remove NAME: delete an alias
    if let Some(alias_name) = remove {
        match alias_store.remove(alias_name) {
            Ok(true) => {
                if ctx.output.json {
                    let json = serde_json::json!({
                        "status": "removed",
                        "alias": alias_name
                    });
                    print_json(&json);
                } else {
                    ctx.output.success(&format!("Removed alias '{alias_name}'"));
                }
            }
            Ok(false) => {
                ctx.output.error(
                    "Alias not found",
                    &format!("No alias named '{alias_name}'"),
                    None,
                    None,
                );
                return ExitCode::from(EXIT_USER_ERROR);
            }
            Err(e) => return handle_error(&e, ctx.output),
        }
        return ExitCode::SUCCESS;
    }

    // Create alias: requires both name and session
    match (name, session) {
        (Some(alias_name), Some(target_session)) => {
            // Validate alias name
            if let Err(e) = validate_alias_name(alias_name) {
                ctx.output
                    .error("Invalid alias name", &e.to_string(), None, None);
                return ExitCode::from(EXIT_USER_ERROR);
            }

            // Warn if alias name shadows an existing session name
            let store = SessionStore::new(ctx.paths.clone());
            let all_ids = store.list().unwrap_or_default();
            for id in &all_ids {
                if let Ok(s) = store.load(id) {
                    if s.name() == alias_name {
                        ctx.output.warning(&format!(
                            "Alias '{alias_name}' shadows session name '{alias_name}'. The alias will take precedence."
                        ));
                        break;
                    }
                }
            }

            if let Err(e) = alias_store.set(alias_name, target_session) {
                return handle_error(&e, ctx.output);
            }

            if ctx.output.json {
                let json = serde_json::json!({
                    "status": "created",
                    "alias": alias_name,
                    "session": target_session
                });
                print_json(&json);
            } else {
                ctx.output
                    .success(&format!("Alias '{alias_name}' -> '{target_session}'"));
            }
            ExitCode::SUCCESS
        }
        (Some(_), None) | (None, Some(_)) => {
            ctx.output.error(
                "Invalid usage",
                "Both alias name and target session are required",
                None,
                Some("Usage: rec alias NAME SESSION\nSee: rec alias --help"),
            );
            ExitCode::from(EXIT_USER_ERROR)
        }
        (None, None) => {
            // No args and no flags -> show usage hint
            ctx.output
                .info("Usage: rec alias NAME SESSION  -- create alias");
            ctx.output
                .info("       rec alias --list        -- list all aliases");
            ctx.output
                .info("       rec alias --remove N    -- remove alias");
            ExitCode::SUCCESS
        }
    }
}
