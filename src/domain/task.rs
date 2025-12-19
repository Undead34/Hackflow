use std::collections::HashMap;
use std::path::PathBuf;

use super::values::{
    CommandOutput, ExecutionContext, LuaScript, PipeStrategy, ShellCommand, TaskId,
};

#[derive(Debug, Clone)]
pub enum TaskAction {
    Command(ShellCommand),
    CreateDir { path: PathBuf },
    Print { message: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    ReadyToRun,
    Running,
    Completed(CommandOutput),
    Failed(String),
    Skipped(String),
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub action: TaskAction,
    pub input_pipes: Vec<PipeStrategy>,
    pub pre_hook: Option<LuaScript>,
    pub post_hook: Option<LuaScript>,
    pub status: TaskStatus,
    pub depends_on: Vec<TaskId>,
}

impl Task {
    pub fn try_transition_to_ready(&mut self, dependencies_met: bool) {
        if matches!(self.status, TaskStatus::Pending) && dependencies_met {
            self.status = TaskStatus::ReadyToRun;
        }
    }

    pub fn resolve_command_params(
        &self,
        context: &ExecutionContext,
    ) -> Option<(ShellCommand, Option<String>)> {
        let base_cmd = match &self.action {
            TaskAction::Command(cmd) => cmd.clone(),
            _ => return None,
        };

        let mut final_cmd = apply_variables_to_command(&base_cmd, &context.variables);
        let mut stdin_buffer: Option<String> = None;

        for pipe in &self.input_pipes {
            match pipe {
                PipeStrategy::StdInFrom(source_id) => {
                    if let Some(output) = context.past_results.get(source_id) {
                        stdin_buffer = Some(output.stdout.clone());
                    }
                }
                PipeStrategy::EnvVarFrom { source, var_name } => {
                    if let Some(output) = context.past_results.get(source) {
                        final_cmd
                            .env_vars
                            .insert(var_name.clone(), output.stdout.clone());
                    }
                }
                PipeStrategy::ArgPositionFrom { source, position } => {
                    if let Some(output) = context.past_results.get(source) {
                        if *position >= final_cmd.args.len() {
                            // Fill missing positions with empty strings
                            final_cmd
                                .args
                                .resize(*position + 1, String::new());
                        }
                        final_cmd.args[*position] = output.stdout.clone();
                    }
                }
            }
        }

        Some((final_cmd, stdin_buffer))
    }
}

fn replace_tokens(input: &str, variables: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in variables {
        let needle = format!("{{{{{}}}}}", key);
        if result.contains(&needle) {
            result = result.replace(&needle, value);
        }
    }
    result
}

fn expand_arg(arg: &str, variables: &HashMap<String, String>) -> Vec<String> {
    let replaced = replace_tokens(arg, variables);
    let trimmed = arg.trim();

    // If the entire arg is a placeholder, allow splitting by whitespace
    if trimmed.starts_with("{{") && trimmed.ends_with("}}") && replaced.contains(' ') {
        return replaced
            .split_whitespace()
            .filter(|part| !part.is_empty())
            .map(|part| part.to_string())
            .collect();
    }

    vec![replaced]
}

fn apply_variables_to_command(
    cmd: &ShellCommand,
    variables: &HashMap<String, String>,
) -> ShellCommand {
    let program = replace_tokens(&cmd.program, variables);

    let mut args = Vec::new();
    for arg in &cmd.args {
        args.extend(expand_arg(arg, variables));
    }

    let mut env_vars = HashMap::new();
    for (key, value) in &cmd.env_vars {
        env_vars.insert(
            replace_tokens(key, variables),
            replace_tokens(value, variables),
        );
    }

    let working_dir = cmd
        .working_dir
        .as_ref()
        .map(|dir| PathBuf::from(replace_tokens(dir.to_string_lossy().as_ref(), variables)));

    ShellCommand {
        program,
        args,
        env_vars,
        working_dir,
    }
}
