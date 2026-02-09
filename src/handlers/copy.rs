use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use super::HandlerContext;
use super::common::{handle_resolve_error, handle_result, print_json};
use rec::error::{EXIT_SYSTEM_ERROR, EXIT_USER_ERROR};

/// Handle the `copy` command — create a copy of an existing session with a new name.
///
/// Copies all commands, tags, and metadata from the source session to a new
/// session with a new UUID and name. The copy is independent of the original.
///
/// # Errors
/// Returns exit code 1 if session not found or name is invalid/already exists.
/// Returns exit code 2 on I/O or serialization errors.
pub fn handle_copy(ctx: &HandlerContext, source: &str, new_name: &str) -> ExitCode {
    use rec::models::validate_session_name;
    use rec::session::resolve_session_with_alias;
    use rec::storage::{AliasStore, SessionStore};

    let store = SessionStore::new(ctx.paths.clone());
    let alias_store = AliasStore::new(&ctx.paths);
    let interactive = rec::replay::prompt::is_interactive();

    // Resolve source session
    let session = match resolve_session_with_alias(&store, &alias_store, source, interactive) {
        Ok(s) => s,
        Err(e) => return handle_resolve_error(&e, ctx.output),
    };

    // Validate new name
    if let Err(e) = validate_session_name(new_name) {
        ctx.output
            .error("Invalid session name", &e.to_string(), None, None);
        return ExitCode::from(EXIT_USER_ERROR);
    }

    // Check for name collision
    let all_ids = match store.list() {
        Ok(ids) => ids,
        Err(e) => {
            ctx.output
                .error("Failed to list sessions", &e.to_string(), None, None);
            return ExitCode::from(EXIT_SYSTEM_ERROR);
        }
    };
    for id in &all_ids {
        if let Ok(s) = store.load(id) {
            if s.name() == new_name {
                ctx.output.error(
                    "Name already exists",
                    &format!(
                        "A session named '{new_name}' already exists. Choose a different name."
                    ),
                    None,
                    None,
                );
                return ExitCode::from(EXIT_USER_ERROR);
            }
        }
    }

    // Create a copy with new UUID and name
    let mut copied_session = session.clone();
    copied_session.header.id = Uuid::new_v4();
    copied_session.header.name = new_name.to_string();
    copied_session.header.started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs_f64();

    // Update footer timestamp if present
    if let Some(ref mut footer) = copied_session.footer {
        footer.ended_at = copied_session.header.started_at;
    }

    // Save the copy
    let result = match store.save(&copied_session) {
        Ok(()) => {
            if ctx.output.json {
                let json = serde_json::json!({
                    "status": "copied",
                    "source": {
                        "id": session.id().to_string(),
                        "name": session.name()
                    },
                    "copy": {
                        "id": copied_session.header.id.to_string(),
                        "name": new_name
                    }
                });
                print_json(&json);
            } else {
                ctx.output
                    .success(&format!("Copied '{}' to '{}'", session.name(), new_name));
            }
            Ok(())
        }
        Err(e) => Err(e),
    };

    handle_result(result, ctx.output, &ctx.paths)
}
