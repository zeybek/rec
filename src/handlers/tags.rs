use super::HandlerContext;
use super::common::handle_result;
use rec::error::EXIT_SYSTEM_ERROR;
use std::process::ExitCode;

/// Handle the `tags` command — lists all tags or normalizes them.
pub fn handle_tags(ctx: &HandlerContext, action: Option<&rec::cli::TagsAction>) -> ExitCode {
    use rec::cli::TagsAction;
    use rec::session::{list_tags, normalize_tag};
    use rec::storage::SessionStore;

    let store = SessionStore::new(ctx.paths.clone());

    let result = match action {
        None => {
            // Existing list_tags behavior (backward compat: `rec tags`)
            list_tags(&store, ctx.output.json, &ctx.output)
        }
        Some(TagsAction::Normalize) => {
            // Normalize all existing tags across all sessions
            let ids = match store.list() {
                Ok(ids) => ids,
                Err(e) => {
                    ctx.output
                        .error("Failed to list sessions", &e.to_string(), None, None);
                    return ExitCode::from(EXIT_SYSTEM_ERROR);
                }
            };
            let mut sessions_modified = 0u32;
            let mut tags_normalized = 0u32;

            for id in &ids {
                if let Ok(mut session) = store.load(id) {
                    let original_tags = session.header.tags.clone();
                    let normalized: Vec<String> = original_tags
                        .iter()
                        .map(|t| normalize_tag(t))
                        .filter(|t| !t.is_empty())
                        .collect();

                    // Deduplicate after normalization
                    let mut deduped: Vec<String> = Vec::new();
                    for tag in &normalized {
                        if !deduped.contains(tag) {
                            deduped.push(tag.clone());
                        }
                    }

                    if deduped != original_tags {
                        session.header.tags = deduped;
                        if let Err(e) = store.save(&session) {
                            ctx.output.warning(&format!(
                                "Failed to save session '{}': {}",
                                session.name(),
                                e
                            ));
                            continue;
                        }
                        sessions_modified += 1;
                        tags_normalized += original_tags.len() as u32;
                    }
                }
            }

            if sessions_modified == 0 {
                ctx.output.info("All tags are already normalized");
            } else {
                ctx.output.success(&format!(
                    "Normalized tags in {sessions_modified} session(s) ({tags_normalized} tags processed)"
                ));
            }
            Ok(())
        }
    };
    handle_result(result, ctx.output, &ctx.paths)
}
