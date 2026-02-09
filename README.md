[![CI](https://github.com/zeybek/rec/actions/workflows/ci.yml/badge.svg)](https://github.com/zeybek/rec/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/zeybek/rec/branch/main/graph/badge.svg)](https://codecov.io/gh/zeybek/rec)
[![crates.io](https://img.shields.io/crates/v/rec-cli)](https://crates.io/crates/rec-cli)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![MSRV: 1.85](https://img.shields.io/badge/MSRV-1.85-orange)](https://www.rust-lang.org)

# rec

A CLI tool to record, replay, and export terminal sessions.

`rec` captures every command you run in a shell session, then lets you replay them with safety controls or export them to scripts, CI/CD configs, and documentation - turning your terminal workflow into reusable artifacts.

## Quick Install

```bash
cargo install rec-cli

# With interactive TUI (optional)
cargo install rec-cli --features tui
```

Pre-built binaries are available for Linux (x86_64, aarch64) and macOS (x86_64, Apple Silicon) on the [releases page](https://github.com/zeybek/rec/releases/latest).

## Quick Start

```bash
# 1. Set up shell hooks (add to your .bashrc, .zshrc, or config.fish)
eval "$(rec init bash)"   # or: zsh, fish

# 2. Record a session
rec start deploy-server
echo "deploying..."
git pull origin main
docker compose up -d
rec stop

# 3. Replay it
rec replay deploy-server              # interactive replay
rec replay deploy-server --dry-run    # preview without executing
rec replay deploy-server --step       # one command at a time

# 4. Export to a reusable format
rec export deploy-server -f bash -o deploy.sh
rec export deploy-server -f github-action -o .github/workflows/deploy.yml
rec export deploy-server -f dockerfile -o Dockerfile
```

## Features

### Recording

`rec` uses native shell hooks to transparently capture every command, its working directory, exit code, and timing -- with zero overhead when not recording.

- **Bash, Zsh, and Fish** support with native hook integration
- **Crash-safe** NDJSON storage with per-command fsync
- **Automatic recovery** of sessions from crashed processes
- **Named sessions** with auto-generated names as fallback

### Replay

Re-execute recorded sessions with a multi-layered safety system that prevents dangerous commands from causing harm.

```bash
rec replay my-session                         # interactive replay
rec replay my-session --dry-run               # preview only
rec replay my-session --step                  # step through one by one
rec replay my-session --skip 3,5              # skip specific commands
rec replay my-session --from 4                # start from command #4
rec replay my-session --skip-pattern "rm*"    # skip by glob pattern
rec replay my-session --cwd                   # use original directories
rec replay my-session --danger-policy skip    # auto-skip dangerous commands
rec replay my-session --danger-policy abort   # abort if any dangerous found
rec replay my-session --danger-policy allow   # allow everything
```

**Safety presets** detect destructive commands across three tiers:
- **Minimal** -- `rm -rf /`, `mkfs`, `dd`, fork bombs
- **Moderate** -- adds `chmod -R 777`, `DROP TABLE`, `docker system prune`, `shutdown`, `kill -9`
- **Strict** -- adds `sudo`, `curl | sh`, `pip install`, `apt remove`

Custom patterns can be added via configuration.

### Export

Convert recorded sessions into 7 reusable formats:

| Format | Command | Produces |
|--------|---------|----------|
| **Bash** | `rec export -f bash` | Shell script with `set -euo pipefail` |
| **Makefile** | `rec export -f makefile` | GNU Makefile with phony targets |
| **Markdown** | `rec export -f markdown` | Documentation with code blocks |
| **GitHub Actions** | `rec export -f github-action` | `.github/workflows/*.yml` |
| **GitLab CI** | `rec export -f gitlab-ci` | `.gitlab-ci.yml` pipeline config |
| **Dockerfile** | `rec export -f dockerfile` | Multi-step Dockerfile |
| **CircleCI** | `rec export -f circleci` | `.circleci/config.yml` |

#### Smart Parameterization

Auto-detect environment-specific values and replace them with variables for portable output:

```bash
rec export my-session -f bash --parameterize
# Detects /home/alice -> $HOME_DIR, alice -> $USERNAME, hostname -> $HOSTNAME

rec export my-session -f bash --param DB_HOST=prod-db.example.com
# Set explicit parameter values
```

Use `{{VAR_NAME}}` placeholders directly in your commands during recording for manual parameterization.

### Session Management

```bash
rec list                          # list all sessions
rec list --tag deploy             # filter by tag
rec list --tag deploy --tag prod --tag-all  # require all tags
rec show my-session               # show session details
rec show my-session --grep "docker"  # filter commands by pattern
rec search "docker compose"       # full-text search across all sessions
rec search --regex "git (push|pull)"  # regex search
rec diff session-1 session-2      # unified diff between sessions
rec rename my-session new-name    # rename a session
rec edit my-session               # edit in $EDITOR (TOML format)
rec tag my-session deploy prod    # add tags
rec tags                          # list all tags with counts
rec tags normalize                # normalize tag casing/format
rec stats                         # aggregate recording statistics
rec delete my-session             # delete a session
rec delete --all                  # delete all sessions
rec delete --all --force          # delete all without confirmation
rec alias deploy deploy-server    # create a short alias
rec status                        # show current recording status
```

### Interactive TUI

`rec` includes an optional terminal user interface for visual session management:

```bash
# Install with TUI support
cargo install rec-cli --features tui

# Launch the TUI
rec ui
```

**Features:**
- Browse and filter sessions with keyboard navigation
- View session details and command history
- Interactive replay with output display
- Export wizard with format selection
- Delete sessions with confirmation modal

**Keybindings:**
| Key | Action |
|-----|--------|
| `j/k` or `^/v` | Navigate up/down |
| `Enter` | Select / Confirm |
| `e` | Export selected session |
| `r` | Replay selected session |
| `d` | Delete selected session |
| `/` | Filter sessions |
| `?` | Toggle help panel |
| `q` | Quit / Back |

### Import

Import existing shell history or scripts into rec sessions:

```bash
rec import deploy.sh                          # auto-detect format
rec import ~/.bash_history --name old-cmds    # import bash history
rec import ~/.zsh_history                     # zsh extended history
rec import ~/.local/share/fish/fish_history   # fish history
```

Supported formats: Bash scripts, Bash history, Zsh history (plain and extended), Fish history.

### Diagnostics

```bash
rec doctor        # run 9 diagnostic checks with fix hints
rec doctor --json # machine-readable output
rec demo          # interactive walkthrough of all features
```

## Configuration

`rec` uses a layered TOML config at `~/.config/rec/config.toml`:

```bash
rec config --list               # show all settings with sources
rec config --get safety.preset  # get a specific value
rec config --set style.colors never  # set a value
rec config --edit               # open in $EDITOR
rec config --path               # print config file path
```

```toml
[general]
editor = "vim"          # or $EDITOR
shell = "bash"          # or $SHELL

[style]
colors = "auto"         # auto | always | never
symbols = "unicode"     # unicode | ascii
verbosity = "normal"    # quiet | normal | verbose

[safety]
preset = "moderate"     # strict | moderate | minimal
custom_patterns = ["kubectl delete", "terraform destroy"]
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `REC_VERBOSE` | Enable verbose output |
| `REC_QUIET` | Suppress non-essential output |
| `NO_COLOR` | Disable colored output ([no-color.org](https://no-color.org)) |
| `REC_EDITOR` | Override editor for `rec edit` and `rec config --edit` |
| `REC_SHELL` | Override detected shell |
| `REC_STORAGE_PATH` | Override session storage directory |

## Shell Completions

```bash
# Generate at runtime
rec completions bash > ~/.local/share/bash-completion/completions/rec
rec completions zsh > ~/.local/share/zsh/site-functions/_rec
rec completions fish > ~/.config/fish/completions/rec.fish
```

Generate completions for your shell and add to your shell configuration.

## How It Works

1. `rec init bash` outputs shell hooks that register `preexec`/`precmd` functions (Bash uses a bundled [bash-preexec](https://github.com/rcaloras/bash-preexec) library; Zsh and Fish use native hooks)
2. `rec start` creates a session file and sets a recording lock with PID-based ownership
3. Shell hooks call `rec _hook preexec <cmd>` before each command and `rec _hook precmd <exit_code>` after, appending NDJSON lines to the session file with fsync
4. `rec stop` writes a footer line and releases the lock
5. Sessions are stored as NDJSON at `~/.local/share/rec/sessions/` -- one JSON object per line, crash-recoverable by design

## Troubleshooting

### Commands not being recorded

**Symptom:** `rec stop` shows "0 commands" even though you ran commands.

**Cause:** Shell hooks are not loaded or `REC_RECORDING` environment variable is not set.

**Solution:**
1. Make sure you've added the init line to your shell rc file:
   ```bash
   # ~/.bashrc, ~/.zshrc, or ~/.config/fish/config.fish
   eval "$(rec init bash)"   # or: zsh, fish
   ```
2. Restart your shell or run `source ~/.bashrc` (or equivalent)
3. Verify hooks are loaded: the `rec` command should be a shell function, not just the binary
   ```bash
   type rec   # should show "rec is a function" not just the path
   ```

### Recording indicator not showing

**Symptom:** No red `●` indicator in prompt during recording.

**Solution:** The prompt indicator requires the shell hooks to be loaded. If you're using a custom prompt theme (oh-my-zsh, starship, etc.), it may override the prompt modification. You can:
1. Manually add `$(__rec_prompt_indicator)` to your prompt
2. Or set `REC_NO_PROMPT=1` to disable the indicator

### Permission denied errors

**Symptom:** Errors about unable to write to session files.

**Solution:** Check permissions on the storage directory:
```bash
ls -la ~/.local/share/rec/
# Should be owned by your user with write permissions
```

### Shell hooks interfering with other tools

**Symptom:** Conflicts with other preexec/precmd hooks or shell plugins.

**Solution:** Load `rec init` after other shell plugins in your rc file. The hooks are designed to coexist with other tools, but load order can matter.

### Run diagnostics

When in doubt, run the built-in diagnostic tool:
```bash
rec doctor
```

This checks 9 common issues including hook installation, storage permissions, and configuration validity.

## License

Licensed under the [MIT License](LICENSE).
