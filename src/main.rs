#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::struct_excessive_bools)]

mod handlers;

use clap::Parser;
use handlers::HandlerContext;
use rec::cli::{Cli, Commands, Output};
use rec::config::load_config;
use rec::error::{EXIT_INTERRUPTED, EXIT_SYSTEM_ERROR};
use rec::storage::Paths;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Install global Ctrl+C handler for exit code 130
    let interrupted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let int_flag = std::sync::Arc::clone(&interrupted);
        if let Err(e) = ctrlc::set_handler(move || {
            int_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }) {
            eprintln!("warning: Failed to set Ctrl-C handler: {e}");
        }
    }

    let cli = Cli::parse();
    let output = Output::new(cli.verbose, cli.quiet, cli.json);

    // Ensure directories exist
    let paths = Paths::new();
    if let Err(e) = paths.ensure_dirs() {
        output.error(
            "Initialization error",
            "Failed to create directories",
            Some(&e.to_string()),
            Some("Check permissions on ~/.local/share and ~/.config"),
        );
        return ExitCode::from(EXIT_SYSTEM_ERROR);
    }

    // Debug: show paths in verbose mode
    output.debug(&format!("Data directory: {}", paths.data_dir.display()));
    output.debug(&format!("Config file: {}", paths.config_file.display()));

    // Load config (errors are non-fatal, use defaults)
    let config = match load_config() {
        Ok(c) => {
            output.debug("Config loaded successfully");
            c
        }
        Err(e) => {
            output.debug(&format!("Using default config: {e}"));
            rec::models::Config::default()
        }
    };

    let ctx = HandlerContext {
        paths,
        config,
        output,
        interrupted: std::sync::Arc::clone(&interrupted),
    };

    // Dispatch to command handlers
    let exit_code = match &cli.command {
        Some(Commands::Start { name }) => handlers::start::handle_start(&ctx, name.as_ref()),
        Some(Commands::Stop) => handlers::stop::handle_stop(&ctx),
        Some(Commands::Replay {
            session: identifier,
            dry_run,
            step,
            skip,
            from,
            force,
            skip_pattern,
            cwd,
            danger_policy,
        }) => {
            let opts = handlers::replay::ReplayOptions {
                dry_run: *dry_run,
                step: *step,
                skip: skip.as_ref(),
                from: *from,
                force: *force,
                skip_pattern: skip_pattern.as_ref(),
                cwd: *cwd,
                danger_policy: *danger_policy,
            };
            handlers::replay::handle_replay(&ctx, identifier, &opts)
        }
        Some(Commands::List { tag, tag_all }) => handlers::list::handle_list(&ctx, tag, *tag_all),
        Some(Commands::Show {
            session: identifier,
            grep,
        }) => handlers::show::handle_show(&ctx, identifier, grep.as_ref()),
        Some(Commands::Delete {
            session: identifier,
            force,
            all,
        }) => handlers::delete::handle_delete(&ctx, identifier.as_deref(), *force, *all),
        Some(Commands::Demo) => handlers::demo::handle_demo(&ctx),
        Some(Commands::Doctor) => handlers::demo::handle_doctor(&ctx),
        Some(Commands::Copy { source, name }) => handlers::copy::handle_copy(&ctx, source, name),
        Some(Commands::Rename { old, new }) => handlers::rename::handle_rename(&ctx, old, new),
        Some(Commands::Edit {
            session: identifier,
        }) => handlers::edit::handle_edit(&ctx, identifier),
        Some(Commands::Tag {
            session: identifier,
            tags,
        }) => handlers::tag::handle_tag(&ctx, identifier, tags),
        Some(Commands::Tags { action }) => handlers::tags::handle_tags(&ctx, action.as_ref()),
        Some(Commands::Export {
            session: identifier,
            format,
            output: output_file,
            parameterize,
            params: param_args,
        }) => handlers::export::handle_export(
            &ctx,
            identifier,
            format,
            output_file.as_ref(),
            *parameterize,
            param_args,
        ),
        Some(Commands::Import { file, name }) => {
            handlers::import::handle_import(&ctx, file, name.as_ref())
        }
        Some(Commands::Init { shell }) => handlers::init::handle_init(&ctx, *shell),
        Some(Commands::Config {
            get,
            set,
            edit,
            path,
            list,
        }) => {
            handlers::config::handle_config(&ctx, get.as_ref(), set.as_ref(), *edit, *path, *list)
        }
        Some(Commands::Hook { hook_type, arg }) => {
            handlers::hook::handle_hook(&ctx, hook_type, arg)
        }
        Some(Commands::Status) => handlers::status::handle_status(&ctx),
        Some(Commands::Search {
            pattern,
            regex,
            tag,
        }) => handlers::search::handle_search(&ctx, pattern, *regex, tag),
        Some(Commands::Diff { session1, session2 }) => {
            handlers::diff::handle_diff(&ctx, session1, session2)
        }
        Some(Commands::Stats) => handlers::stats::handle_stats(&ctx),
        Some(Commands::Alias {
            name,
            session,
            list,
            remove,
        }) => handlers::alias::handle_alias(
            &ctx,
            name.as_ref(),
            session.as_ref(),
            *list,
            remove.as_ref(),
        ),
        Some(Commands::Completions { shell }) => handlers::completions::handle_completions(*shell),
        Some(Commands::Ui) => handlers::ui::handle_ui(&ctx),
        None => handlers::help::handle_none(),
    };

    // Check if interrupted by Ctrl+C
    if interrupted.load(std::sync::atomic::Ordering::SeqCst) {
        return ExitCode::from(EXIT_INTERRUPTED);
    }

    exit_code
}
