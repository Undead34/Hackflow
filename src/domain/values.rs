use std::collections::HashMap;
use std::path::PathBuf;

pub type TaskId = String;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LuaScript(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommand {
    pub program: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub past_results: HashMap<TaskId, CommandOutput>,
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum PipeStrategy {
    StdInFrom(TaskId),
    EnvVarFrom { source: TaskId, var_name: String },
    ArgPositionFrom { source: TaskId, position: usize },
}
