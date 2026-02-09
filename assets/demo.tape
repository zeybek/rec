# VHS Demo Tape for rec - Terminal Session Recorder
# https://github.com/charmbracelet/vhs

Output assets/demo.gif

Set Shell zsh
Set FontFamily "JetBrains Mono"
Set FontSize 13
Set Width 1000
Set Height 600
Set Padding 15
Set Theme "GruvboxDark"
Set TypingSpeed 70ms

# === SET CLEAN PROMPT ===
Hide
Type "PS1='%F{green}>%f '"
Enter
Type `eval "$(rec init zsh)"`
Enter
Sleep 0.5s
Show

# === INTRO ===
Type "rec --help"
Enter
Sleep 4s
Ctrl+L

# === RECORDING A SESSION ===
Type "rec start --name my-project"
Enter
Sleep 2s

Type "echo 'Hello from rec!'"
Enter
Sleep 1.5s

Type "whoami"
Enter
Sleep 1.5s

Type "pwd"
Enter
Sleep 1.5s

Type "ls -la"
Enter
Sleep 2s

Type "rec stop"
Enter
Sleep 3s
Ctrl+L

# === LIST & SHOW ===
Type "rec list"
Enter
Sleep 3s

Type "rec show my-project"
Enter
Sleep 4s
Ctrl+L

# === TAGGING ===
Type "rec tag my-project demo tutorial"
Enter
Sleep 2s

Type "rec tags"
Enter
Sleep 3s
Ctrl+L

# === SEARCH ===
Type "rec search echo"
Enter
Sleep 3s
Ctrl+L

# === STATS ===
Type "rec stats"
Enter
Sleep 4s
Ctrl+L

# === EXPORT FORMATS ===
Type "rec export my-project -f bash"
Enter
Sleep 4s
Ctrl+L

Type "rec export my-project -f makefile"
Enter
Sleep 4s
Ctrl+L

Type "rec export my-project -f github-action"
Enter
Sleep 4s
Ctrl+L

Type "rec export my-project -f dockerfile"
Enter
Sleep 4s
Ctrl+L

Type "rec export my-project -f markdown"
Enter
Sleep 4s
Ctrl+L

# === REPLAY ===
Type "rec replay my-project --dry-run"
Enter
Sleep 4s
Ctrl+L

# === ALIAS ===
Type "rec alias mp my-project"
Enter
Sleep 2s

Type "rec show mp"
Enter
Sleep 3s
Ctrl+L

# === COPY & DIFF ===
Type "rec copy my-project backup"
Enter
Sleep 2s

Type "rec diff my-project backup"
Enter
Sleep 3s
Ctrl+L

# === DOCTOR ===
Type "rec doctor"
Enter
Sleep 4s
Ctrl+L

# === CONFIG ===
Type "rec config list"
Enter
Sleep 3s
Ctrl+L

# === SHELL INTEGRATION ===
Type "rec init zsh | head -15"
Enter
Sleep 3s
Ctrl+L

# === FINAL ===
Type "rec list"
Enter
Sleep 3s

Type "echo 'cargo install rec-cli'"
Enter
Sleep 4s
