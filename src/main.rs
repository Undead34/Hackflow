mod flow;

use std::env;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("./examples/osint_workflow.lua")
    };

    flow::Workflow::new(file_path)?.execute(args)?;

    Ok(())
}
