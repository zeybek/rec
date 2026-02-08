use std::process::ExitCode;

/// Handle the `None` case — no subcommand provided, show brief help.
pub fn handle_none() -> ExitCode {
    println!("rec - Record, replay, and export terminal sessions");
    println!();
    println!("Usage: rec <COMMAND>");
    println!();
    println!("Commands:");
    println!("  start       Start recording a new session");
    println!("  stop        Stop the current recording");
    println!("  replay      Replay a recorded session");
    println!("  list        List all recorded sessions");
    println!("  show        Show details of a session");
    println!("  delete      Delete a session");
    println!("  demo        Run an interactive walkthrough");
    println!("  doctor      Diagnose installation issues");
    println!("  rename      Rename a session");
    println!("  edit        Edit a session in $EDITOR");
    println!("  tag         Add a tag to a session");
    println!("  import      Import from scripts or history");
    println!("  export      Export a session to another format");
    println!("  search      Search across all sessions");
    println!("  diff        Compare commands between sessions");
    println!("  stats       Show recording statistics");
    println!("  alias       Manage session aliases");
    println!("  status      Show current recording status");
    println!("  init        Initialize shell hooks");
    println!("  config      Show or modify configuration");
    println!("  completions Generate shell completions");
    println!();
    println!("Run 'rec --help' for more information");
    ExitCode::SUCCESS
}
