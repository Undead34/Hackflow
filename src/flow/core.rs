use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use anyhow::Result;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

use super::task::{Task, TaskHandle, TaskNode, TaskOutput, TaskStatus};

/// Represents a workflow execution engine
#[derive(Debug)]
pub struct Flow {
    /// Next available task handle
    next_handle: TaskHandle,
    /// Map of task handles to task nodes
    tasks: HashMap<TaskHandle, TaskNode>,
    /// Directed Acyclic Graph (DAG) of tasks
    dag: DiGraph<TaskHandle, ()>,
    /// Map of task handles to DAG node indices
    node_indices: HashMap<TaskHandle, NodeIndex>,
}

impl Flow {
    pub fn new() -> Self {
        Self {
            next_handle: 1,
            tasks: HashMap::new(),
            dag: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }

    /// Generate a new unique task handle
    fn generate_handle(&mut self) -> TaskHandle {
        let handle = self.next_handle;
        self.next_handle += 1;
        handle
    }

    /// Add a task to the workflow
    fn add_task(&mut self, task: Task, dependencies: Option<HashSet<TaskHandle>>) -> TaskHandle {
        let handle = self.generate_handle();
        let dependencies = dependencies.unwrap_or_default();
        
        // Create the task node
        let task_node = TaskNode {
            handle,
            task,
            dependencies: dependencies.clone(),
            status: TaskStatus::Pending,
            output: TaskOutput::None,
        };
        
        // Add the task to the DAG
        let node_idx = self.dag.add_node(handle);
        self.node_indices.insert(handle, node_idx);
        
        // Add edges for dependencies
        for dep_handle in &dependencies {
            if let Some(&dep_idx) = self.node_indices.get(dep_handle) {
                self.dag.add_edge(dep_idx, node_idx, ());
            }
        }
        
        // Store the task
        self.tasks.insert(handle, task_node);
        
        handle
    }

    /// Execute all tasks in the workflow sequentially
    /// This method blocks until all tasks are completed
    pub fn execute(&mut self) -> bool {
        if self.tasks.is_empty() {
            return true;
        }

        // Execute tasks in the order they were added
        let handles: Vec<TaskHandle> = self.tasks.iter()
            .filter(|(_, node)| !matches!(node.status, TaskStatus::Completed))
            .map(|(handle, _)| *handle)
            .collect();
        
        if handles.is_empty() {
            return true; // All tasks are already completed
        }
        
        for handle in handles {
            if !self.execute_task(handle) {
                return false;
            }
        }

        // Verify all tasks were executed
        self.tasks.values().all(|node| matches!(node.status, TaskStatus::Completed))
    }

    /// Execute all tasks in the workflow in parallel, respecting dependencies
    /// This method blocks until all tasks are completed
    pub fn execute_parallel(&mut self) -> bool {
        if self.tasks.is_empty() {
            return true;
        }

        // Find all root nodes (tasks with no dependencies)
        let mut ready_tasks: VecDeque<TaskHandle> = self.tasks
            .values()
            .filter(|node| node.dependencies.is_empty() && !matches!(node.status, TaskStatus::Completed))
            .map(|node| node.handle)
            .collect();

        // Track completed tasks
        let mut completed = HashSet::new();
        
        // Add already completed tasks to the completed set
        for (handle, node) in &self.tasks {
            if matches!(node.status, TaskStatus::Completed) {
                completed.insert(*handle);
            }
        }
        
        // Process tasks in topological order
        while !ready_tasks.is_empty() {
            let handle = ready_tasks.pop_front().unwrap();
            
            // Skip if already completed
            if completed.contains(&handle) {
                continue;
            }
            
            // Execute the task
            if !self.execute_task(handle) {
                return false;
            }
            
            // Mark as completed
            completed.insert(handle);
            
            // Find tasks that are now ready to execute
            if let Some(&node_idx) = self.node_indices.get(&handle) {
                // Get all successors (tasks that depend on this one)
                for successor in self.dag.neighbors_directed(node_idx, Direction::Outgoing) {
                    let successor_handle = self.dag[successor];
                    
                    // Check if all dependencies are satisfied
                    if let Some(task_node) = self.tasks.get(&successor_handle) {
                        if !completed.contains(&successor_handle) && 
                           task_node.dependencies.iter().all(|dep| completed.contains(dep)) {
                            ready_tasks.push_back(successor_handle);
                        }
                    }
                }
            }
        }

        // Check if all tasks were executed
        let all_completed = self.tasks.values().all(|node| 
            matches!(node.status, TaskStatus::Completed));
            
        // Reset any tasks that weren't executed (this shouldn't happen with a proper DAG)
        if !all_completed {
            println!("Warning: Some tasks were not executed. The workflow may have cycles.");
            
            // Identify tasks that weren't completed
            let pending_tasks: Vec<TaskHandle> = self.tasks.iter()
                .filter(|(_, node)| !matches!(node.status, TaskStatus::Completed))
                .map(|(handle, _)| *handle)
                .collect();
                
            for handle in pending_tasks {
                if let Some(task) = self.tasks.get(&handle) {
                    println!("Pending task: {}", task.task);
                }
            }
        }
        
        all_completed
    }

    /// Execute a single task
    fn execute_task(&mut self, handle: TaskHandle) -> bool {
        if let Some(mut task_node) = self.tasks.get(&handle).cloned() {
            // Update status to running
            task_node.status = TaskStatus::Running;
            self.tasks.insert(handle, task_node.clone());
            
            println!("Executing task: {}", task_node.task);
            
            // Get input from dependencies if needed
            let input = self.get_input_for_task(&task_node);
            
            // Execute the task based on its type
            let result = match &task_node.task {
                Task::RunCommand { command, args, .. } => {
                    // Implementation for running commands
                    println!("Running command: {} {:?}", command, args);
                    // Placeholder for actual command execution
                    TaskOutput::String(format!("Output from command: {}", command))
                },
                Task::Print { message } => {
                    println!("{}", message);
                    TaskOutput::None
                },
                Task::DnsLookup { domain, .. } => {
                    // Placeholder for DNS lookup implementation
                    println!("DNS lookup for: {}", domain);
                    TaskOutput::DnsLookup(vec![format!("192.168.1.1 for {}", domain)])
                },
                Task::RunNmap { targets, options, .. } => {
                    // Placeholder for Nmap implementation
                    println!("Running Nmap on: {:?} with options: {:?}", targets, options);
                    TaskOutput::Nmap(serde_json::json!({"hosts": targets}))
                },
                Task::RunSubfinder { domain, options, .. } => {
                    // Placeholder for Subfinder implementation
                    println!("Running Subfinder on: {} with options: {:?}", domain, options);
                    TaskOutput::Subfinder(vec![format!("sub.{}", domain)])
                },
                Task::RunWappalyzer { url, .. } => {
                    // Placeholder for Wappalyzer implementation
                    println!("Running Wappalyzer on: {}", url);
                    TaskOutput::Json(serde_json::json!({"technologies": ["Apache", "PHP"]}))
                },
            };
            
            // Update task with result
            task_node.status = TaskStatus::Completed;
            task_node.output = result;
            self.tasks.insert(handle, task_node);
            
            true
        } else {
            println!("Task not found: {}", handle);
            false
        }
    }
    
    /// Get input for a task from its dependencies
    fn get_input_for_task(&self, task_node: &TaskNode) -> Option<TaskOutput> {
        let input_handle = match &task_node.task {
            Task::RunCommand { input_handle, .. } => input_handle,
            Task::DnsLookup { input_handle, .. } => input_handle,
            Task::RunNmap { input_handle, .. } => input_handle,
            Task::RunSubfinder { input_handle, .. } => input_handle,
            Task::RunWappalyzer { input_handle, .. } => input_handle,
            _ => &None,
        };
        
        if let Some(handle) = input_handle {
            if let Some(input_task) = self.tasks.get(handle) {
                return Some(input_task.output.clone());
            }
        }
        
        None
    }
}

impl Flow {
    /// Add a generic command task to the workflow
    pub fn run_command(&mut self, command: String, args: Option<Vec<String>>, input_handle: Option<TaskHandle>) -> TaskHandle {
        let task = Task::RunCommand { 
            command, 
            args, 
            input_handle 
        };
        
        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }
        
        self.add_task(task, Some(dependencies))
    }

    /// Add a print task to the workflow
    pub fn print(&mut self, message: String) -> TaskHandle {
        let task = Task::Print { message };
        self.add_task(task, None)
    }
    
    /// Add a DNS lookup task to the workflow
    pub fn dns_lookup(&mut self, domain: String, input_handle: Option<TaskHandle>) -> TaskHandle {
        let task = Task::DnsLookup { 
            domain, 
            input_handle 
        };
        
        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }
        
        self.add_task(task, Some(dependencies))
    }
    
    /// Add an Nmap scan task to the workflow
    pub fn run_nmap(&mut self, targets: Vec<String>, options: Option<Vec<String>>, input_handle: Option<TaskHandle>) -> TaskHandle {
        let task = Task::RunNmap { 
            targets, 
            options, 
            input_handle 
        };
        
        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }
        
        self.add_task(task, Some(dependencies))
    }
    
    /// Add a Subfinder task to the workflow
    pub fn run_subfinder(&mut self, domain: String, options: Option<Vec<String>>, input_handle: Option<TaskHandle>) -> TaskHandle {
        let task = Task::RunSubfinder { 
            domain, 
            options, 
            input_handle 
        };
        
        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }
        
        self.add_task(task, Some(dependencies))
    }
    
    /// Add a Wappalyzer task to the workflow
    pub fn run_wappalyzer(&mut self, url: String, input_handle: Option<TaskHandle>) -> TaskHandle {
        let task = Task::RunWappalyzer { 
            url, 
            input_handle 
        };
        
        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }
        
        self.add_task(task, Some(dependencies))
    }
    
    /// Get the output of a task
    pub fn get_output(&self, handle: TaskHandle) -> Option<TaskOutput> {
        self.tasks.get(&handle).map(|task| task.output.clone())
    }
}
