use std::collections::HashMap;

use super::task::{Task, TaskStatus};
use super::values::{CommandOutput, TaskId};

pub struct Workflow {
    tasks: HashMap<TaskId, Task>,
}

impl Workflow {
    pub fn new(tasks: Vec<Task>) -> Self {
        let mut map = HashMap::new();
        for task in tasks {
            map.insert(task.id.clone(), task);
        }
        Self { tasks: map }
    }

    pub fn get_executable_tasks(&mut self) -> Vec<TaskId> {
        let mut ready = Vec::new();
        let keys: Vec<TaskId> = self.tasks.keys().cloned().collect();

        for task_id in keys {
            let Some(task) = self.tasks.get(&task_id) else {
                continue;
            };

            if !matches!(task.status, TaskStatus::Pending) {
                continue;
            }

            let (deps_met, blocked_reason) = self.dependencies_met(task);

            if let Some(reason) = blocked_reason {
                if let Some(task_mut) = self.tasks.get_mut(&task_id) {
                    task_mut.status = TaskStatus::Skipped(reason);
                }
                continue;
            }

            if deps_met {
                if let Some(task_mut) = self.tasks.get_mut(&task_id) {
                    task_mut.try_transition_to_ready(true);
                }
                ready.push(task_id);
            }
        }

        ready
    }

    pub fn mark_running(&mut self, task_id: &TaskId) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = TaskStatus::Running;
        }
    }

    pub fn mark_completed(&mut self, task_id: &TaskId, output: CommandOutput) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = TaskStatus::Completed(output);
        }
    }

    pub fn mark_failed(&mut self, task_id: &TaskId, error: String) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = TaskStatus::Failed(error);
        }
    }

    pub fn mark_skipped(&mut self, task_id: &TaskId, reason: String) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = TaskStatus::Skipped(reason);
        }
    }

    pub fn get_task(&self, task_id: &TaskId) -> Option<&Task> {
        self.tasks.get(task_id)
    }

    pub fn tasks(&self) -> &HashMap<TaskId, Task> {
        &self.tasks
    }

    pub fn all_finished(&self) -> bool {
        self.tasks.values().all(|task| {
            matches!(
                task.status,
                TaskStatus::Completed(_) | TaskStatus::Failed(_) | TaskStatus::Skipped(_)
            )
        })
    }

    fn dependencies_met(&self, task: &Task) -> (bool, Option<String>) {
        let mut has_failed_dependency: Option<String> = None;

        for dep_id in &task.depends_on {
            let Some(dep_task) = self.tasks.get(dep_id) else {
                has_failed_dependency =
                    Some(format!("Dependency '{dep_id}' not found for task '{}'", task.id));
                break;
            };

            match &dep_task.status {
                TaskStatus::Completed(_) => continue,
                TaskStatus::Skipped(reason) => {
                    has_failed_dependency = Some(format!(
                        "Dependency '{dep_id}' skipped: {reason}"
                    ));
                    break;
                }
                TaskStatus::Failed(reason) => {
                    has_failed_dependency =
                        Some(format!("Dependency '{dep_id}' failed: {reason}"));
                    break;
                }
                _ => return (false, None),
            }
        }

        if let Some(reason) = has_failed_dependency {
            return (false, Some(reason));
        }

        (true, None)
    }
}
