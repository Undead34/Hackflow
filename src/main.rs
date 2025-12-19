mod application;
mod domain;
mod infrastructure;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand};
use colored::*;
use domain::{ExecutionContext, TaskStatus};
use infrastructure::{
    load_workflow_from_path, LuaScriptEngine, SystemCommandExecutor,
};

use application::WorkflowRunner;

#[derive(Parser)]
#[command(name = "hackflow", version, author)]
struct App {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init(InitArgs),
    Workflow(WorkflowArgs),
}

#[derive(Args)]
struct InitArgs {
    /// Nombre del workflow
    name: String,
    /// Descripción opcional
    description: Option<String>,
}

#[derive(Args)]
struct WorkflowArgs {
    /// Ruta del workflow (JSON). Se buscará también en ./workflows y en workflows junto al binario.
    path: PathBuf,
    /// Variables dinámicas (formato clave=valor)
    #[arg(short = 'v', long = "var", value_parser = parse_key_val::<String, String>)]
    variables: Vec<(String, String)>,
    /// Argumentos a pasar al workflow (se exportan como variable {{targets}})
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

fn parse_key_val<K, V>(s: &str) -> std::result::Result<(K, V), String>
where
    K: std::str::FromStr,
    V: std::str::FromStr,
    K::Err: std::fmt::Display,
    V::Err: std::fmt::Display,
{
    let pos = s
        .find('=')
        .ok_or_else(|| "Las variables deben tener formato clave=valor".to_string())?;
    let key: K = s[..pos]
        .parse()
        .map_err(|e: K::Err| e.to_string())?;
    let value: V = s[pos + 1..]
        .parse()
        .map_err(|e: V::Err| e.to_string())?;
    Ok((key, value))
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = App::parse();

    match app.command {
        Commands::Init(args) => handle_init(args)?,
        Commands::Workflow(args) => handle_workflow(args).await?,
    }

    Ok(())
}

fn handle_init(args: InitArgs) -> Result<()> {
    let sanitized = sanitise_file_name::sanitise(&args.name);
    let workflow_dir = PathBuf::from(&sanitized);

    fs::create_dir_all(&workflow_dir)
        .with_context(|| format!("No se pudo crear el directorio {}", workflow_dir.display()))?;

    let description = args.description.unwrap_or_default();
    let state = format!(
        "name = \"{}\"\npath = \"{}\"\ndescription = \"{}\"",
        args.name,
        workflow_dir.canonicalize()?.display(),
        description
    );
    fs::write(workflow_dir.join(".workflow"), state)
        .context("No se pudo escribir el archivo .workflow")?;

    println!("{}", "✅ HackFlow".bright_green().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());
    println!("{} {}", "Workflow:".bright_yellow().bold(), args.name.bright_white());
    if !description.is_empty() {
        println!(
            "{} {}",
            "Description:".bright_yellow().bold(),
            description.bright_white()
        );
    }
    println!(
        "{} {}",
        "Location:".bright_yellow().bold(),
        workflow_dir.display().to_string().bright_white()
    );
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());

    Ok(())
}

async fn handle_workflow(args: WorkflowArgs) -> Result<()> {
    let workflow_path = resolve_workflow_path(&args.path)
        .with_context(|| format!("Workflow file {} not found", args.path.display()))?;

    let mut context = ExecutionContext::default();
    context
        .variables
        .insert("targets".to_string(), args.args.join(" "));

    for (key, value) in args.variables {
        context.variables.insert(key, value);
    }

    let loaded = load_workflow_from_path(&workflow_path)?;

    println!("{}", "🚀 HackFlow".bright_green().bold());
    if let Some(name) = loaded.name {
        println!("{} {}", "Workflow:".bright_yellow().bold(), name.bright_white());
    }
    if let Some(desc) = loaded.description {
        println!(
            "{} {}",
            "Description:".bright_yellow().bold(),
            desc.bright_white()
        );
    }
    println!(
        "{} {}",
        "File:".bright_yellow().bold(),
        workflow_path.display().to_string().bright_white()
    );
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());

    let executor = Arc::new(SystemCommandExecutor::new());
    let script_engine = Arc::new(LuaScriptEngine::new());

    let runner = WorkflowRunner::new(loaded.workflow, context, executor, script_engine);
    let report = runner.run().await;

    for (task_id, status) in &report.tasks {
        let display_name = report
            .task_names
            .get(task_id)
            .cloned()
            .unwrap_or_else(|| task_id.clone());
        match status {
            TaskStatus::Completed(_) => println!("✅ {display_name} ({task_id})"),
            TaskStatus::Failed(err) => println!("❌ {display_name} ({task_id}) - {err}"),
            TaskStatus::Skipped(reason) => {
                println!("⚠️  {display_name} ({task_id}) - {reason}")
            }
            TaskStatus::Pending => println!("⏳ {display_name} pending"),
            TaskStatus::ReadyToRun => println!("⏳ {display_name} ready"),
            TaskStatus::Running => println!("⏳ {display_name} running"),
        }
    }

    println!(
        "{} {}",
        "Context results:".bright_yellow().bold(),
        report.context.past_results.len()
    );

    if report.success {
        println!("{}", "✅ Workflow completed successfully".bright_green().bold());
    } else {
        println!("{}", "❌ Workflow finished with issues".bright_red().bold());
    }

    Ok(())
}

fn resolve_workflow_path(input: &Path) -> Result<PathBuf> {
    if input.exists() {
        return Ok(input.to_path_buf());
    }

    let mut candidates = Vec::new();

    let with_extension = if input.extension().is_some() {
        input.to_path_buf()
    } else {
        let mut p = input.to_path_buf();
        p.set_extension("json");
        p
    };

    let current_dir = std::env::current_dir()?;
    candidates.push(current_dir.join(&with_extension));

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            candidates.push(parent.join("workflows").join(&with_extension));
        }
    }

    candidates.push(PathBuf::from("workflows").join(&with_extension));

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(anyhow!("No workflow found matching {}", input.display()))
}
