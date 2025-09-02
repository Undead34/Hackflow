mod config;
mod flow;

use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

/// CLI entry point for Hackflow
#[derive(Parser)]
#[command(author, version, about = "Motor de orquestación de flujos", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available CLI commands
#[derive(Subcommand)]
enum Commands {
    /// Ejecuta un flujo definido en un archivo Lua
    Run {
        /// Ruta al archivo del flujo
        file: PathBuf,
        /// Argumentos adicionales a pasar al flujo
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Crea la estructura básica de trabajo y un archivo de configuración
    Init {
        /// Directorio donde inicializar la estructura (por defecto el actual)
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { file, args } => run_workflow(file, args)?,
        Commands::Init { path } => init_workspace(path)?,
    }

    Ok(())
}

fn run_workflow(file: PathBuf, args: Vec<String>) -> anyhow::Result<()> {
    let mut wf_args = vec![std::env::args().next().unwrap_or_else(|| "hackflow".into())];
    wf_args.push(file.to_string_lossy().into_owned());
    wf_args.extend(args);

    flow::Workflow::new(file)?.execute(wf_args)?;
    Ok(())
}

fn init_workspace(path: PathBuf) -> anyhow::Result<()> {
    let flows_dir = path.join("flows");
    fs::create_dir_all(&flows_dir)?;

    let config_path = path.join("config.toml");
    if !config_path.exists() {
        fs::write(&config_path, DEFAULT_CONFIG)?;
    }

    println!("Inicializado Hackflow en {}", path.display());
    Ok(())
}

const DEFAULT_CONFIG: &str = "[cli_paths]\ndnsrecon = \"dnsrecon\"\nwsl = \"wsl\"\n";
