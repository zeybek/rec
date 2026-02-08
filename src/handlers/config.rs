use super::HandlerContext;
use super::common::{handle_result, print_json};
use rec::config::ConfigLoader;
use rec::error::{EXIT_SYSTEM_ERROR, RecError};
use std::process::ExitCode;

pub fn handle_config(
    ctx: &HandlerContext,
    get: Option<&String>,
    set: Option<&Vec<String>>,
    edit: bool,
    path: bool,
    list: bool,
) -> ExitCode {
    let loader = ConfigLoader::new(ctx.paths.clone());

    if path {
        println!("{}", ctx.paths.config_file.display());
        return ExitCode::SUCCESS;
    }

    if list {
        // --list: show all config values with source annotations
        let result = match loader.list_config() {
            Ok(values) => {
                if ctx.output.json {
                    let json_values: Vec<serde_json::Value> = values
                        .iter()
                        .map(|v| {
                            serde_json::json!({
                                "key": v.key,
                                "value": v.value,
                                "source": v.source.to_string(),
                            })
                        })
                        .collect();
                    print_json(&serde_json::Value::Array(json_values));
                } else {
                    // Find max key + value width for alignment
                    let max_kv_len = values
                        .iter()
                        .map(|v| v.key.len() + v.value.len() + 3) // " = "
                        .max()
                        .unwrap_or(0);

                    for v in &values {
                        let kv = format!("{} = {}", v.key, v.value);
                        let padding = max_kv_len.saturating_sub(kv.len());
                        let source_str = match &v.source {
                            rec::config::ConfigSource::Default => "default".to_string(),
                            rec::config::ConfigSource::File => "file".to_string(),
                            rec::config::ConfigSource::Env(var) => {
                                format!(
                                    "env: {}={}",
                                    var,
                                    v.env_override.as_ref().map_or("", |(_, val)| val.as_str())
                                )
                            }
                        };
                        println!("{}{}  # {}", kv, " ".repeat(padding), source_str);
                    }
                }
                Ok(())
            }
            Err(e) => Err(e),
        };
        return handle_result(result, ctx.output, &ctx.paths);
    }

    if edit {
        // --edit: open config in $EDITOR with validation re-edit loop
        if let Err(e) = loader.create_default_if_missing() {
            ctx.output.error("Config error", &e.to_string(), None, None);
            return ExitCode::from(EXIT_SYSTEM_ERROR);
        }

        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_string());

        let mut edit_result: std::result::Result<(), RecError> = Ok(());
        loop {
            let status = match std::process::Command::new(&editor)
                .arg(&ctx.paths.config_file)
                .stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status()
            {
                Ok(s) => s,
                Err(e) => {
                    ctx.output.error(
                        "Editor error",
                        &format!("Failed to launch '{editor}': {e}"),
                        None,
                        Some("Set $EDITOR or $VISUAL to your preferred editor"),
                    );
                    return ExitCode::from(EXIT_SYSTEM_ERROR);
                }
            };

            if !status.success() {
                edit_result = Err(RecError::Config("Editor exited with error".into()));
                break;
            }

            // Validate
            let contents = match std::fs::read_to_string(&ctx.paths.config_file) {
                Ok(c) => c,
                Err(e) => {
                    edit_result = Err(RecError::Io(e));
                    break;
                }
            };
            match toml::from_str::<rec::models::Config>(&contents) {
                Ok(_) => {
                    ctx.output.success("Config saved successfully");
                    break;
                }
                Err(e) => {
                    ctx.output
                        .error("Config validation failed", &e.to_string(), None, None);

                    if rec::replay::prompt::is_interactive() {
                        let re_edit = dialoguer::Confirm::new()
                            .with_prompt("Re-edit config file?")
                            .default(true)
                            .interact()
                            .unwrap_or(false);

                        if !re_edit {
                            edit_result =
                                Err(RecError::Config("Config file may contain errors".into()));
                            break;
                        }
                        // Loop continues — re-open editor
                    } else {
                        edit_result = Err(RecError::Toml(e));
                        break;
                    }
                }
            }
        }
        return handle_result(edit_result, ctx.output, &ctx.paths);
    }

    if let Some(key) = get {
        // --get KEY: show value with env override annotation
        let result = match loader.get_key(key) {
            Ok(cv) => {
                println!("{}", cv.value);
                if let Some((ref env_var, ref env_val)) = cv.env_override {
                    eprintln!("  # overridden by {env_var}={env_val}");
                    if let Some(ref file_val) = cv.file_value {
                        eprintln!("  # file value: {file_val}");
                    }
                }
                Ok(())
            }
            Err(e) => Err(e),
        };
        return handle_result(result, ctx.output, &ctx.paths);
    }

    if let Some(args) = set {
        // --set KEY VALUE: write value preserving TOML comments
        let key = &args[0];
        let value = &args[1];
        let result = match loader.set_key(key, value) {
            Ok(()) => {
                ctx.output.success(&format!("Set {key} = {value}"));
                Ok(())
            }
            Err(e) => Err(e),
        };
        return handle_result(result, ctx.output, &ctx.paths);
    }

    // No flags: show usage hint
    println!("Config file: {}", ctx.paths.config_file.display());
    println!(
        "Use --list to see all values, --edit to edit, --get KEY to read, --set KEY VALUE to write"
    );
    ExitCode::SUCCESS
}
