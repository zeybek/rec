use super::HandlerContext;
use super::common::{handle_resolve_error, handle_result, print_json};
use rec::error::EXIT_USER_ERROR;
use std::process::ExitCode;

/// Handle the `tag` command — add tags to a session.
pub fn handle_tag(ctx: &HandlerContext, identifier: &str, tags: &[String]) -> ExitCode {
    use rec::recording::RecordingState;
    use rec::session::{
        find_tag_collision, normalize_tag, resolve_session_with_alias, validate_tag_name,
    };
    use rec::storage::{AliasStore, SessionStore};

    let store = SessionStore::new(ctx.paths.clone());
    let alias_store = AliasStore::new(&ctx.paths);
    let interactive = rec::replay::prompt::is_interactive();
    let session = match resolve_session_with_alias(&store, &alias_store, identifier, interactive) {
        Ok(s) => s,
        Err(e) => return handle_resolve_error(&e, ctx.output),
    };

    // Check if session is currently being recorded
    let recording_state = RecordingState::new(&ctx.paths.state_dir);
    if recording_state.is_recording() {
        if let Ok(active) = recording_state.current() {
            let session_path = ctx.paths.session_file(&session.id().to_string());
            if active.session_path == session_path {
                ctx.output.error(
                    "Cannot modify session",
                    "Cannot modify session while recording is in progress",
                    None,
                    Some("Stop recording first with: rec stop"),
                );
                return ExitCode::from(EXIT_USER_ERROR);
            }
        }
    }

    // Normalize tags before storing
    let normalized_tags: Vec<String> = tags.iter().map(|t| normalize_tag(t)).collect();

    // Print normalization/collision notes
    let existing_tags = session.header.tags.clone();
    for (original, normalized) in tags.iter().zip(&normalized_tags) {
        if original != normalized {
            if find_tag_collision(normalized, &existing_tags).is_some() {
                ctx.output.info(&format!(
                    "note: Using existing tag '{normalized}' (normalized from '{original}')"
                ));
            } else {
                ctx.output.info(&format!(
                    "note: Tag normalized: '{original}' → '{normalized}'"
                ));
            }
        }
    }

    // Filter out empty normalized tags and validate each one
    let normalized_tags: Vec<String> = normalized_tags
        .into_iter()
        .filter(|t| !t.is_empty())
        .collect();
    if normalized_tags.is_empty() {
        ctx.output.error(
            "Invalid tags",
            "All provided tags are empty after normalization",
            None,
            None,
        );
        return ExitCode::from(EXIT_USER_ERROR);
    }

    // Validate all normalized tags before proceeding
    for tag in &normalized_tags {
        if let Err(e) = validate_tag_name(tag) {
            ctx.output
                .error("Invalid tag name", &e.to_string(), None, None);
            return ExitCode::from(EXIT_USER_ERROR);
        }
    }

    let session_id = session.id().to_string();
    let session_name = session.name().to_string();

    let result = match store.add_tags(&session_id, normalized_tags.clone()) {
        Ok(all_tags) => {
            // Calculate which tags were actually new
            let added: Vec<&String> = normalized_tags
                .iter()
                .filter(|t| !existing_tags.iter().any(|et| &normalize_tag(et) == *t))
                .collect();

            if ctx.output.json {
                let json = serde_json::json!({
                    "status": "tagged",
                    "session": {
                        "id": session_id,
                        "name": session_name
                    },
                    "tags_added": added,
                    "tags": all_tags
                });
                print_json(&json);
            } else {
                if added.is_empty() {
                    ctx.output.info(&format!(
                        "No new tags added to '{session_name}' (all already present)"
                    ));
                } else {
                    let added_str: Vec<&str> = added.iter().map(|s| s.as_str()).collect();
                    ctx.output.success(&format!(
                        "Added tags to '{}': {}",
                        session_name,
                        added_str.join(", ")
                    ));
                }
                ctx.output
                    .info(&format!("All tags: {}", all_tags.join(", ")));
            }
            Ok(())
        }
        Err(e) => {
            let backup_path = ctx.paths.backup_file(&session_id);
            if backup_path.exists() {
                ctx.output.info(&format!(
                    "note: Backup preserved at {}",
                    backup_path.display()
                ));
            }
            Err(e)
        }
    };
    handle_result(result, ctx.output, &ctx.paths)
}
