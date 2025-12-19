use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::domain::{
    LuaScript, PipeStrategy, ShellCommand, Task, TaskAction, TaskStatus, Workflow,
};

pub struct LoadedWorkflow {
    pub name: Option<String>,
    pub description: Option<String>,
    pub workflow: Workflow,
}

#[derive(Deserialize)]
struct WorkflowFile {
    name: Option<String>,
    description: Option<String>,
    tasks: Vec<TaskFile>,
}

#[derive(Deserialize)]
struct TaskFile {
    id: String,
    name: Option<String>,
    action: ActionFile,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    pipes: Vec<PipeFile>,
    pre_hook: Option<String>,
    post_hook: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ActionFile {
    Command {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        cwd: Option<String>,
    },
    CreateDir { path: String },
    Print { message: String },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PipeFile {
    StdinFrom { task: String },
    EnvFrom { task: String, var: String },
    ArgFrom { task: String, position: usize },
}

impl From<PipeFile> for PipeStrategy {
    fn from(value: PipeFile) -> Self {
        match value {
            PipeFile::StdinFrom { task } => PipeStrategy::StdInFrom(task),
            PipeFile::EnvFrom { task, var } => PipeStrategy::EnvVarFrom {
                source: task,
                var_name: var,
            },
            PipeFile::ArgFrom { task, position } => PipeStrategy::ArgPositionFrom {
                source: task,
                position,
            },
        }
    }
}

impl TryFrom<TaskFile> for Task {
    type Error = anyhow::Error;

    fn try_from(value: TaskFile) -> Result<Self> {
        let action = match value.action {
            ActionFile::Command {
                program,
                args,
                env,
                cwd,
            } => TaskAction::Command(ShellCommand {
                program,
                args,
                env_vars: env,
                working_dir: cwd.map(PathBuf::from),
            }),
            ActionFile::CreateDir { path } => TaskAction::CreateDir { path: PathBuf::from(path) },
            ActionFile::Print { message } => TaskAction::Print { message },
        };

        let input_pipes = value.pipes.into_iter().map(PipeStrategy::from).collect();

        Ok(Task {
            id: value.id.clone(),
            name: value.name.unwrap_or_else(|| value.id.clone()),
            action,
            input_pipes,
            pre_hook: value.pre_hook.map(LuaScript),
            post_hook: value.post_hook.map(LuaScript),
            status: TaskStatus::Pending,
            depends_on: value.depends_on,
        })
    }
}

pub fn load_workflow_from_path(path: &Path) -> Result<LoadedWorkflow> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read workflow file {}", path.display()))?;

    let workflow_file: WorkflowFile =
        serde_json::from_str(&content).context("Failed to parse workflow JSON")?;

    let mut tasks = Vec::new();
    for task_file in workflow_file.tasks {
        tasks.push(task_file.try_into()?);
    }

    Ok(LoadedWorkflow {
        name: workflow_file.name,
        description: workflow_file.description,
        workflow: Workflow::new(tasks),
    })
}
