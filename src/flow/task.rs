use std::collections::HashSet;
use std::fmt::{self, Debug, Formatter};
use std::path::PathBuf;

/// Unique identifier for a task
pub type TaskHandle = u64;

/// Status of a task in the workflow
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,
    /// Task is currently running
    Running,
    /// Task has completed successfully
    Completed,
    /// Task has failed
    Failed(String),
}

/// Output from a task that can be passed to other tasks
#[derive(Clone, Debug)]
pub enum TaskOutput {
    /// No output
    None,
    /// String output
    String(String),
    // Boolean output
    Bool(bool),
    /// JSON output
    Json(serde_json::Value),
    /// Binary output
    Binary(Vec<u8>),
    /// DNS lookup output
    DnsLookup {
        csv_file: PathBuf,
        json_file: PathBuf,
        exit_code: i32,
        stdout: String,
        stderr: String,
    },
    /// Subfinder output (domains)
    Subfinder(Vec<String>),
    /// Nmap output (scan results)
    Nmap(serde_json::Value),
}

/// A task in the workflow
pub struct TaskNode {
    /// Unique handle for this task
    pub handle: TaskHandle,
    /// Task executor
    pub executor: Box<dyn crate::flow::task_executor::TaskExecutor + Send + Sync>,
    /// Dependencies (handles of tasks that must complete before this one)
    pub dependencies: HashSet<TaskHandle>,
    /// Current status of the task
    pub status: TaskStatus,
    /// Output of the task after execution
    pub output: TaskOutput,
}

impl Debug for TaskNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaskNode")
            .field("handle", &self.handle)
            .field("executor_name", &self.executor.name())
            .field("dependencies", &self.dependencies)
            .field("status", &self.status)
            .field("output", &self.output)
            .finish()
    }
}

// Implementación manual de Clone para TaskNode
// No podemos usar #[derive(Clone)] porque Box<dyn TaskExecutor> no implementa Clone automáticamente
impl Clone for TaskNode {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            executor: self.executor.clone_box(),
            dependencies: self.dependencies.clone(),
            status: self.status.clone(),
            output: self.output.clone(),
        }
    }
}
