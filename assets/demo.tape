# VHS Demo Tape for rec
# https://github.com/charmbracelet/vhs

Output assets/demo.gif

Set Shell bash
Set FontSize 14
Set Width 900
Set Height 500
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 40ms

# Title
Type "# rec - Record, Replay, Export terminal sessions"
Enter
Sleep 1.5s

# Start recording
Type "rec start my-session"
Enter
Sleep 1s

# Simple commands that won't produce long output
Type "echo 'Hello World'"
Enter
Sleep 800ms

Type "whoami"
Enter
Sleep 800ms

Type "date"
Enter
Sleep 800ms

Type "pwd"
Enter
Sleep 800ms

# Stop recording
Type "rec stop"
Enter
Sleep 1.2s

# List sessions
Type "rec list"
Enter
Sleep 1.5s

# Show session details
Type "rec show my-session"
Enter
Sleep 2s

# Export formats
Type "# Export to various formats:"
Enter
Sleep 500ms

Type "rec export my-session -f bash"
Enter
Sleep 2s

Type "rec export my-session -f makefile"
Enter
Sleep 2s

Type "rec export my-session -f github-action"
Enter
Sleep 2.5s

# Replay
Type "# Replay with safety checks:"
Enter
Sleep 500ms

Type "rec replay my-session --dry-run"
Enter
Sleep 2s

# Search
Type "rec search echo"
Enter
Sleep 1.5s

# Cleanup
Type "rec delete my-session --force"
Enter
Sleep 1s

# Final
Type "# Install: cargo install rec-cli"
Enter
Sleep 2s
