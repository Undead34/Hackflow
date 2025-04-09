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

        // Get the next available handle to determine the number of tasks added so far
        let max_handle = self.next_handle - 1;
        
        // Execute tasks in the order they were added (by handle value)
        // Handles are assigned sequentially starting from 1
        let mut handles: Vec<TaskHandle> = (1..=max_handle)
            .filter(|handle| self.tasks.contains_key(handle) && 
                   !matches!(self.tasks.get(handle).unwrap().status, TaskStatus::Completed))
            .collect();
        
        if handles.is_empty() {
            return true; // All tasks are already completed
        }
        
        // Sort by handle to ensure sequential execution in the order tasks were added
        handles.sort();
        
        for handle in handles {
            if !self.execute_task(handle) {
                return false;
            }
        }

        // Verify all tasks were executed
        self.tasks.values().all(|node| matches!(node.status, TaskStatus::Completed))
    }

    /// Execute a single task asynchronously
    async fn execute_task_async(
        handle: TaskHandle,
        tasks: Arc<Mutex<HashMap<TaskHandle, TaskNode>>>,
        completed: Arc<Mutex<HashSet<TaskHandle>>>,
    ) -> bool {
        // Get the task
        let mut task_node = {
            let tasks_guard = tasks.lock().await;
            if let Some(node) = tasks_guard.get(&handle) {
                node.clone()
            } else {
                println!("Task not found: {}", handle);
                return false;
            }
        };
        
        // Update status to running
        task_node.status = TaskStatus::Running;
        {
            let mut tasks_guard = tasks.lock().await;
            tasks_guard.insert(handle, task_node.clone());
        }
        
        // println!("Executing task: {}", task_node.task);
        
        // Get input from dependencies if needed
        let input = {
            let tasks_guard = tasks.lock().await;
            Self::get_input_for_task_async(&task_node, &tasks_guard)
        };
        
        // Execute the task based on its type
        let result = match &task_node.task {
            Task::RunCommand { command, args, .. } => {
                // Implementation for running commands
                println!("Running command: {} {:?}", command, args);
                // Placeholder for actual command execution
                // In a real implementation, you would use tokio::process::Command
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; // Simulate work
                TaskOutput::String(format!("Output from command: {}", command))
            },
            Task::Print { message } => {
                println!("{}", message);
                TaskOutput::None
            },
            Task::DnsLookup { domain, .. } => {
                // Placeholder for DNS lookup implementation
                println!("DNS lookup for: {}", domain);
                // In a real implementation, you would use tokio::net::lookup_host
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await; // Simulate work
                TaskOutput::DnsLookup(vec![format!("192.168.1.1 for {}", domain)])
            },
            Task::RunNmap { targets, options, .. } => {
                // Placeholder for Nmap implementation
                println!("Running Nmap on: {:?} with options: {:?}", targets, options);
                // In a real implementation, you would use tokio::process::Command
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await; // Simulate work
                TaskOutput::Nmap(serde_json::json!({"hosts": targets}))
            },
            Task::RunSubfinder { domain, options, .. } => {
                // Placeholder for Subfinder implementation
                println!("Running Subfinder on: {} with options: {:?}", domain, options);
                // In a real implementation, you would use tokio::process::Command
                tokio::time::sleep(tokio::time::Duration::from_millis(250)).await; // Simulate work
                TaskOutput::Subfinder(vec![format!("sub.{}", domain)])
            },
            Task::RunWappalyzer { url, .. } => {
                // Placeholder for Wappalyzer implementation
                println!("Running Wappalyzer on: {}", url);
                // In a real implementation, you would use reqwest
                tokio::time::sleep(tokio::time::Duration::from_millis(150)).await; // Simulate work
                TaskOutput::Json(serde_json::json!({"technologies": ["Apache", "PHP"]}))
            },
        };
        
        // Update task with result
        task_node.status = TaskStatus::Completed;
        task_node.output = result;
        
        {
            let mut tasks_guard = tasks.lock().await;
            tasks_guard.insert(handle, task_node);
        }
        
        // Mark as completed
        {
            let mut completed_guard = completed.lock().await;
            completed_guard.insert(handle);
        }
        
        true
    }
    
    /// Get input for a task from its dependencies asynchronously
    fn get_input_for_task_async(task_node: &TaskNode, tasks: &HashMap<TaskHandle, TaskNode>) -> Option<TaskOutput> {
        let input_handle = match &task_node.task {
            Task::RunCommand { input_handle, .. } => input_handle,
            Task::DnsLookup { input_handle, .. } => input_handle,
            Task::RunNmap { input_handle, .. } => input_handle,
            Task::RunSubfinder { input_handle, .. } => input_handle,
            Task::RunWappalyzer { input_handle, .. } => input_handle,
            _ => &None,
        };
        
        if let Some(handle) = input_handle {
            if let Some(input_task) = tasks.get(handle) {
                return Some(input_task.output.clone());
            }
        }
        
        None
    }

    /// Execute all tasks in the workflow in parallel using tokio
    /// This is the async implementation that does the actual parallel execution
    async fn execute_parallel_async(&self) -> bool {
        if self.tasks.is_empty() {
            return true;
        }
        
        // Create shared state for tasks and completed set
        let tasks = Arc::new(Mutex::new(self.tasks.clone()));
        let completed = Arc::new(Mutex::new(HashSet::new()));
        
        // Add already completed tasks to the completed set
        {
            let mut completed_guard = completed.lock().await;
            for (handle, node) in &self.tasks {
                if matches!(node.status, TaskStatus::Completed) {
                    completed_guard.insert(*handle);
                }
            }
        }
        
        // Create a semaphore to limit concurrency (adjust the number based on your needs)
        let semaphore = Arc::new(Semaphore::new(4)); // Allow 4 concurrent tasks
        
        // Create a set to track all spawned tasks
        let mut join_set = JoinSet::new();
        
        // Find all root nodes (tasks with no dependencies)
        let root_tasks: Vec<TaskHandle> = self.tasks
            .values()
            .filter(|node| node.dependencies.is_empty() && !matches!(node.status, TaskStatus::Completed))
            .map(|node| node.handle)
            .collect();
        
        // Start with root tasks
        for handle in root_tasks {
            let tasks_clone = Arc::clone(&tasks);
            let completed_clone = Arc::clone(&completed);
            let semaphore_clone = Arc::clone(&semaphore);
            
            join_set.spawn(async move {
                // Acquire a permit from the semaphore
                let _permit = semaphore_clone.acquire().await.unwrap();
                Self::execute_task_async(handle, tasks_clone, completed_clone).await
            });
        }
        
        // Process remaining tasks as they become ready
        while let Some(result) = join_set.join_next().await {
            // Check if the task succeeded
            if let Ok(success) = result {
                if !success {
                    // If any task fails, abort
                    return false;
                }
            } else {
                // Task panicked
                return false;
            }
            
            // Find tasks that are now ready to execute
            let mut ready_tasks = Vec::new();
            {
                let completed_guard = completed.lock().await;
                let tasks_guard = tasks.lock().await;
                
                for (handle, node) in tasks_guard.iter() {
                    // Skip tasks that are already completed or in progress
                    if completed_guard.contains(handle) || matches!(node.status, TaskStatus::Running) {
                        continue;
                    }
                    
                    // Check if all dependencies are satisfied
                    if node.dependencies.iter().all(|dep| completed_guard.contains(dep)) {
                        ready_tasks.push(*handle);
                    }
                }
            }
            
            // Spawn new tasks for those that are ready
            for handle in ready_tasks {
                let tasks_clone = Arc::clone(&tasks);
                let completed_clone = Arc::clone(&completed);
                let semaphore_clone = Arc::clone(&semaphore);
                
                join_set.spawn(async move {
                    // Acquire a permit from the semaphore
                    let _permit = semaphore_clone.acquire().await.unwrap();
                    Self::execute_task_async(handle, tasks_clone, completed_clone).await
                });
            }
        }
        
        // Check if all tasks were executed
        let all_completed = {
            let tasks_guard = tasks.lock().await;
            tasks_guard.values().all(|node| matches!(node.status, TaskStatus::Completed))
        };
        
        // Report any tasks that weren't executed
        if !all_completed {
            println!("Warning: Some tasks were not executed. The workflow may have cycles.");
            
            let tasks_guard = tasks.lock().await;
            let pending_tasks: Vec<TaskHandle> = tasks_guard.iter()
                .filter(|(_, node)| !matches!(node.status, TaskStatus::Completed))
                .map(|(handle, _)| *handle)
                .collect();
                
            for handle in pending_tasks {
                if let Some(task) = tasks_guard.get(&handle) {
                    println!("Pending task: {}", task.task);
                }
            }
        }
        
        // Return the final result
        all_completed
    }

    /// Execute all tasks in the workflow in parallel, respecting dependencies
    /// This method blocks until all tasks are completed
    /// This is a synchronous wrapper around the async implementation
    pub fn execute_parallel(&mut self) -> bool {
        // Create a new tokio runtime
        let runtime = tokio::runtime::Runtime::new().unwrap();
        
        // Execute the async function and get the result
        let tasks = Arc::new(Mutex::new(self.tasks.clone()));
        
        let result = runtime.block_on(async {
            // Execute all tasks
            let success = self.execute_parallel_async().await;
            
            // If successful, update the tasks in the shared state
            if success {
                // Copy the updated tasks back to self.tasks
                let tasks_guard = tasks.lock().await;
                for (handle, node) in tasks_guard.iter() {
                    self.tasks.insert(*handle, node.clone());
                }
            }
            
            success
        });
        
        result
    }

    /// Execute a single task
    fn execute_task(&mut self, handle: TaskHandle) -> bool {
        if let Some(mut task_node) = self.tasks.get(&handle).cloned() {
            // Update status to running
            task_node.status = TaskStatus::Running;
            self.tasks.insert(handle, task_node.clone());
            
            // println!("Executing task: {}", task_node.task);
            
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
