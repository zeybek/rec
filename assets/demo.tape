# VHS Demo Tape for rec
# https://github.com/charmbracelet/vhs

Output assets/demo.gif

Set Shell zsh
Set FontSize 14
Set Width 900
Set Height 500
Set Padding 20
Set Theme "Catppuccin Mocha"

# Start recording
Type "# Start a new recording session"
Sleep 500ms
Enter
Sleep 300ms

Type "rec start --name deploy-script"
Enter
Sleep 1s

# Run some commands
Type "# Record your terminal commands..."
Sleep 500ms
Enter
Sleep 300ms

Type "echo 'Building project...'"
Enter
Sleep 800ms

Type "cargo build --release"
Sleep 500ms
Enter
Sleep 1s

Type "echo 'Running tests...'"
Enter
Sleep 800ms

Type "cargo test"
Sleep 500ms
Enter
Sleep 1s

# Stop recording
Type "# Stop and save the session"
Sleep 500ms
Enter
Sleep 300ms

Type "rec stop"
Enter
Sleep 1s

# Show the session
Type "# View your recorded session"
Sleep 500ms
Enter
Sleep 300ms

Type "rec show deploy-script"
Enter
Sleep 2s

# Export to bash script
Type "# Export to a bash script"
Sleep 500ms
Enter
Sleep 300ms

Type "rec export deploy-script --format bash"
Enter
Sleep 2s

# Export to GitHub Actions
Type "# Or export to GitHub Actions workflow"
Sleep 500ms
Enter
Sleep 300ms

Type "rec export deploy-script --format github-action"
Enter
Sleep 3s

Type "# That's rec! Record once, export anywhere."
Enter
Sleep 2s
