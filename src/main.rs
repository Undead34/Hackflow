mod flow;

use std::env;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        // Default test workflow path
        PathBuf::from("./examples/test_workflow.lua")
    };

    // Create the workflow
    flow::Workflow::new(file_path)?.execute(args)?;

    Ok(())
}
