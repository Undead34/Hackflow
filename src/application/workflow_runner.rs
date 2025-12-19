use std::collections::HashMap;
use std::sync::Arc;

use tokio::fs;
use tokio::sync::mpsc;

use crate::domain::{
    CommandExecutorPort, CommandOutput, ExecutionContext, ScriptEnginePort, TaskAction, TaskId,
    TaskStatus, Workflow,
};

#[derive(Debug, Clone)]
pub struct WorkflowReport {
    pub context: ExecutionContext,
    pub tasks: HashMap<TaskId, TaskStatus>,
    pub task_names: HashMap<TaskId, String>,
    pub success: bool,
}

enum WorkflowMsg {
    Start,
    TaskCompleted { task_id: TaskId, result: CommandOutput },
    TaskFailed { task_id: TaskId, error: String },
    TaskSkipped { task_id: TaskId, reason: String },
}

pub struct WorkflowRunner {
    workflow: Workflow,
    context: ExecutionContext,
    cmd_executor: Arc<dyn CommandExecutorPort>,
    script_engine: Arc<dyn ScriptEnginePort>,
    sender: mpsc::Sender<WorkflowMsg>,
    receiver: mpsc::Receiver<WorkflowMsg>,
}

impl WorkflowRunner {
    pub fn new(
        workflow: Workflow,
        context: ExecutionContext,
        cmd_executor: Arc<dyn CommandExecutorPort>,
        script_engine: Arc<dyn ScriptEnginePort>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(64);
        Self {
            workflow,
            context,
            cmd_executor,
            script_engine,
            sender,
            receiver,
        }
    }

    pub async fn run(mut self) -> WorkflowReport {
        let _ = self.sender.send(WorkflowMsg::Start).await;

        while let Some(msg) = self.receiver.recv().await {
            match msg {
                WorkflowMsg::Start => self.schedule_next_tasks().await,
                WorkflowMsg::TaskCompleted { task_id, result } => {
                    self.context
                        .past_results
                        .insert(task_id.clone(), result.clone());
                    self.workflow.mark_completed(&task_id, result);
                    self.schedule_next_tasks().await;
                }
                WorkflowMsg::TaskFailed { task_id, error } => {
                    self.workflow.mark_failed(&task_id, error);
                    self.schedule_next_tasks().await;
                }
                WorkflowMsg::TaskSkipped { task_id, reason } => {
                    self.workflow.mark_skipped(&task_id, reason);
                    self.schedule_next_tasks().await;
                }
            }

            if self.workflow.all_finished() {
                break;
            }
        }

        let mut tasks: HashMap<TaskId, TaskStatus> = HashMap::new();
        let mut task_names: HashMap<TaskId, String> = HashMap::new();

        for (id, task) in self.workflow.tasks().iter() {
            tasks.insert(id.clone(), task.status.clone());
            task_names.insert(id.clone(), task.name.clone());
        }

        let success = tasks
            .values()
            .all(|status| !matches!(status, TaskStatus::Failed(_)));

        WorkflowReport {
            context: self.context,
            tasks,
            task_names,
            success,
        }
    }

    async fn schedule_next_tasks(&mut self) {
        let ready_tasks = self.workflow.get_executable_tasks();

        for task_id in ready_tasks {
            let Some(task) = self.workflow.get_task(&task_id).cloned() else {
                continue;
            };

            self.workflow.mark_running(&task_id);

            let context_clone = self.context.clone();
            let executor = self.cmd_executor.clone();
            let script_engine = self.script_engine.clone();
            let sender = self.sender.clone();

            tokio::spawn(async move {
                let pre_hook_allows = match &task.pre_hook {
                    Some(script) => script_engine
                        .evaluate_condition(script, &context_clone)
                        .await,
                    None => true,
                };

                if !pre_hook_allows {
                    let _ = sender
                        .send(WorkflowMsg::TaskSkipped {
                            task_id: task.id.clone(),
                            reason: "Pre-hook condition returned false".to_string(),
                        })
                        .await;
                    return;
                }

                match task.action {
                    TaskAction::Command(_) => {
                        let Some((cmd, stdin_buffer)) =
                            task.resolve_command_params(&context_clone)
                        else {
                            let _ = sender
                                .send(WorkflowMsg::TaskFailed {
                                    task_id: task.id.clone(),
                                    error: "Invalid command parameters".to_string(),
                                })
                                .await;
                            return;
                        };

                        let execution_result = executor.execute(&cmd, stdin_buffer.as_deref()).await;

                        match execution_result {
                            Ok(mut output) => {
                                if let Some(script) = &task.post_hook {
                                    let transformed = script_engine
                                        .transform_output(script, &output.stdout, &context_clone)
                                        .await;
                                    output.stdout = transformed;
                                }

                                if output.exit_code == 0 {
                                    let _ = sender
                                        .send(WorkflowMsg::TaskCompleted {
                                            task_id: task.id.clone(),
                                            result: output,
                                        })
                                        .await;
                                } else {
                                    let message = if output.stderr.is_empty() {
                                        format!(
                                            "Command exited with code {}",
                                            output.exit_code
                                        )
                                    } else {
                                        output.stderr.clone()
                                    };

                                    let _ = sender
                                        .send(WorkflowMsg::TaskFailed {
                                            task_id: task.id.clone(),
                                            error: message,
                                        })
                                        .await;
                                }
                            }
                            Err(err) => {
                                let _ = sender
                                    .send(WorkflowMsg::TaskFailed {
                                        task_id: task.id.clone(),
                                        error: err,
                                    })
                                    .await;
                            }
                        }
                    }
                    TaskAction::CreateDir { path } => {
                        let path_string = replace_tokens_with_variables(
                            &path.to_string_lossy(),
                            &context_clone,
                        );

                        let result = match fs::create_dir_all(&path_string).await {
                            Ok(_) => CommandOutput {
                                stdout: path_string.clone(),
                                stderr: String::new(),
                                exit_code: 0,
                            },
                            Err(err) => CommandOutput {
                                stdout: String::new(),
                                stderr: err.to_string(),
                                exit_code: 1,
                            },
                        };

                        if result.exit_code == 0 {
                            let _ = sender
                                .send(WorkflowMsg::TaskCompleted {
                                    task_id: task.id.clone(),
                                    result,
                                })
                                .await;
                        } else {
                            let _ = sender
                                .send(WorkflowMsg::TaskFailed {
                                    task_id: task.id.clone(),
                                    error: result.stderr,
                                })
                                .await;
                        }
                    }
                    TaskAction::Print { message } => {
                        let final_message =
                            replace_tokens_with_variables(&message, &context_clone);
                        println!("{final_message}");

                        let result = CommandOutput {
                            stdout: final_message,
                            stderr: String::new(),
                            exit_code: 0,
                        };

                        let _ = sender
                            .send(WorkflowMsg::TaskCompleted {
                                task_id: task.id.clone(),
                                result,
                            })
                            .await;
                    }
                }
            });
        }
    }
}

fn replace_tokens_with_variables(text: &str, context: &ExecutionContext) -> String {
    let mut result = text.to_string();
    for (key, value) in &context.variables {
        let needle = format!("{{{{{}}}}}", key);
        if result.contains(&needle) {
            result = result.replace(&needle, value);
        }
    }
    result
}
