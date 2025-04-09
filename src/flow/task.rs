use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

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
    /// JSON output
    Json(serde_json::Value),
    /// Binary output
    Binary(Vec<u8>),
    /// DNS lookup output
    DnsLookup(Vec<String>),
    /// Subfinder output (domains)
    Subfinder(Vec<String>),
    /// Nmap output (scan results)
    Nmap(serde_json::Value),
}

/// A task in the workflow
#[derive(Clone, Debug)]
pub struct TaskNode {
    /// Unique handle for this task
    pub handle: TaskHandle,
    /// Task type and configuration
    pub task: Task,
    /// Dependencies (handles of tasks that must complete before this one)
    pub dependencies: HashSet<TaskHandle>,
    /// Current status of the task
    pub status: TaskStatus,
    /// Output of the task after execution
    pub output: TaskOutput,
}

/// Types of tasks that can be executed in the workflow
#[derive(Clone, Debug)]
pub enum Task {
    /// Run a generic command
    RunCommand {
        command: String,
        args: Option<Vec<String>>,
        input_handle: Option<TaskHandle>,
    },
    /// Print a message
    Print {
        message: String,
    },
    /// DNS lookup
    DnsLookup {
        domain: String,
        input_handle: Option<TaskHandle>,
    },
    /// Run Nmap scan
    RunNmap {
        targets: Vec<String>,
        options: Option<Vec<String>>,
        input_handle: Option<TaskHandle>,
    },
    /// Run Subfinder
    RunSubfinder {
        domain: String,
        options: Option<Vec<String>>,
        input_handle: Option<TaskHandle>,
    },
    /// Run Wappalyzer
    RunWappalyzer {
        url: String,
        input_handle: Option<TaskHandle>,
    },
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Task::RunCommand { command, args, input_handle } => {
                write!(f, "Run command '{}': {:?} (input: {:?})", command, args, input_handle)
            },
            Task::Print { message } => {
                write!(f, "Print: {}", message)
            },
            Task::DnsLookup { domain, input_handle } => {
                write!(f, "DNS Lookup: {} (input: {:?})", domain, input_handle)
            },
            Task::RunNmap { targets, options, input_handle } => {
                write!(f, "Nmap scan: {:?} with options {:?} (input: {:?})", targets, options, input_handle)
            },
            Task::RunSubfinder { domain, options, input_handle } => {
                write!(f, "Subfinder: {} with options {:?} (input: {:?})", domain, options, input_handle)
            },
            Task::RunWappalyzer { url, input_handle } => {
                write!(f, "Wappalyzer: {} (input: {:?})", url, input_handle)
            },
        }
    }
}
