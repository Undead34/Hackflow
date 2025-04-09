use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use petgraph::graph::{DiGraph, NodeIndex};
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

use super::task::{TaskHandle, TaskNode, TaskOutput, TaskStatus};
use super::task_executor::TaskExecutor;
use super::tasks::dns_lookup_task::DnsLookupTask;
use super::tasks::print_task::PrintTask;
use super::tasks::run_command_task::RunCommandTask;
use super::tasks::CreateDirTask;

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
    fn add_task<T>(&mut self, executor: T, dependencies: Option<HashSet<TaskHandle>>) -> TaskHandle
    where
        T: TaskExecutor + 'static + Send + Sync,
    {
        let handle = self.generate_handle();
        let dependencies = dependencies.unwrap_or_default();

        // Create the task node
        let task_node = TaskNode {
            handle,
            executor: Box::new(executor),
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
            .filter(|handle| {
                self.tasks.contains_key(handle)
                    && !matches!(
                        self.tasks.get(handle).unwrap().status,
                        TaskStatus::Completed
                    )
            })
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
        self.tasks
            .values()
            .all(|node| matches!(node.status, TaskStatus::Completed))
    }

    /// Execute a single task asynchronously
    async fn execute_task_async(
        handle: TaskHandle,
        tasks: Arc<Mutex<HashMap<TaskHandle, TaskNode>>>,
        completed: Arc<Mutex<HashSet<TaskHandle>>>,
    ) -> bool {
        // Verificar si la tarea existe
        {
            let tasks_guard = tasks.lock().await;
            if !tasks_guard.contains_key(&handle) {
                println!("Task not found: {}", handle);
                return false;
            }
        }

        // Actualizar el estado a Running
        {
            let mut tasks_guard = tasks.lock().await;
            if let Some(task) = tasks_guard.get_mut(&handle) {
                task.status = TaskStatus::Running;
            }
        }

        // Obtener el input_handle y procesar la entrada si es necesario
        let input_output = {
            let tasks_guard = tasks.lock().await;
            if let Some(task) = tasks_guard.get(&handle) {
                if let Some(input_handle) = task.executor.input_handle() {
                    if let Some(input_task) = tasks_guard.get(&input_handle) {
                        Some(input_task.output.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Procesar la entrada si está disponible
        if let Some(input) = &input_output {
            let mut tasks_guard = tasks.lock().await;
            if let Some(task) = tasks_guard.get_mut(&handle) {
                let _ = task.executor.process_input(input);
            }
        }

        // Ejecutar la tarea
        let result = {
            let tasks_guard = tasks.lock().await;
            if let Some(task) = tasks_guard.get(&handle) {
                task.executor.execute().await
            } else {
                return false;
            }
        };

        // Actualizar la tarea con el resultado
        {
            let mut tasks_guard = tasks.lock().await;
            if let Some(task) = tasks_guard.get_mut(&handle) {
                task.status = TaskStatus::Completed;
                task.output = result;
            }
        }

        // Mark as completed
        {
            let mut completed_guard = completed.lock().await;
            completed_guard.insert(handle);
        }

        true
    }

    /// Execute all tasks in the workflow in parallel using tokio
    /// This is the async implementation that does the actual parallel execution
    async fn execute_parallel_async(&self) -> bool {
        if self.tasks.is_empty() {
            return true;
        }

        // Crear un nuevo HashMap con los mismos valores
        let mut tasks_map = HashMap::new();
        for (handle, node) in &self.tasks {
            // Crear un nuevo TaskNode con los mismos valores
            let new_node = TaskNode {
                handle: node.handle,
                executor: node.executor.clone_box(),
                dependencies: node.dependencies.clone(),
                status: node.status.clone(),
                output: node.output.clone(),
            };
            tasks_map.insert(*handle, new_node);
        }

        // Create shared state for tasks and completed set
        let tasks = Arc::new(Mutex::new(tasks_map));
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
        let root_tasks: Vec<TaskHandle> = self
            .tasks
            .values()
            .filter(|node| {
                node.dependencies.is_empty() && !matches!(node.status, TaskStatus::Completed)
            })
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
                    if completed_guard.contains(handle)
                        || matches!(node.status, TaskStatus::Running)
                    {
                        continue;
                    }

                    // Check if all dependencies are satisfied
                    if node
                        .dependencies
                        .iter()
                        .all(|dep| completed_guard.contains(dep))
                    {
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
            tasks_guard
                .values()
                .all(|node| matches!(node.status, TaskStatus::Completed))
        };

        // Report any tasks that weren't executed
        if !all_completed {
            println!("Warning: Some tasks were not executed. The workflow may have cycles.");

            let tasks_guard = tasks.lock().await;
            let pending_tasks: Vec<TaskHandle> = tasks_guard
                .iter()
                .filter(|(_, node)| !matches!(node.status, TaskStatus::Completed))
                .map(|(handle, _)| *handle)
                .collect();

            for handle in pending_tasks {
                if let Some(task) = tasks_guard.get(&handle) {
                    println!("Pending task: {}", task.executor.name());
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

        let result = runtime.block_on(async {
            // Execute all tasks
            let success = self.execute_parallel_async().await;
            success
        });

        result
    }

    /// Execute a single task
    fn execute_task(&mut self, handle: TaskHandle) -> bool {
        // Verificar si la tarea existe
        if !self.tasks.contains_key(&handle) {
            println!("Task not found: {}", handle);
            return false;
        }

        // Actualizar el estado a Running
        if let Some(task) = self.tasks.get_mut(&handle) {
            task.status = TaskStatus::Running;
        }

        // Obtener el input_handle y procesar la entrada si es necesario
        let input_output = {
            if let Some(task) = self.tasks.get(&handle) {
                if let Some(input_handle) = task.executor.input_handle() {
                    if let Some(input_task) = self.tasks.get(&input_handle) {
                        Some(input_task.output.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Procesar la entrada si está disponible
        if let Some(input) = &input_output {
            if let Some(task) = self.tasks.get_mut(&handle) {
                let _ = task.executor.process_input(input);
            }
        }

        // Ejecutar la tarea usando un runtime de tokio
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async {
            if let Some(task) = self.tasks.get(&handle) {
                task.executor.execute().await
            } else {
                return TaskOutput::None;
            }
        });

        // Actualizar la tarea con el resultado
        if let Some(task) = self.tasks.get_mut(&handle) {
            task.status = TaskStatus::Completed;
            task.output = result;
        }

        true
    }
}

impl Flow {
    /// Add a generic command task to the workflow
    pub fn run_command(
        &mut self,
        command: String,
        args: Option<Vec<String>>,
        input_handle: Option<TaskHandle>,
    ) -> TaskHandle {
        let args = args.unwrap_or_default();
        let task = RunCommandTask::new(command, args);

        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }

        self.add_task(task, Some(dependencies))
    }

    /// Add a print task to the workflow
    pub fn print(&mut self, message: String) -> TaskHandle {
        let task = PrintTask::new(message);
        self.add_task(task, None)
    }

    /// Add a DNS lookup task to the workflow
    pub fn dns_lookup(&mut self, domain: String, args: Option<Vec<String>>, replace_args: Option<bool>, input_handle: Option<TaskHandle>) -> TaskHandle {
        let task = DnsLookupTask::new(domain, args, replace_args);

        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }

        self.add_task(task, Some(dependencies))
    }

    /// Add an Nmap scan task to the workflow
    pub fn run_nmap(
        &mut self,
        targets: Vec<String>,
        options: Option<Vec<String>>,
        input_handle: Option<TaskHandle>,
    ) -> TaskHandle {
        // Implementar NmapTask cuando sea necesario
        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }

        // Por ahora, usamos RunCommandTask como un placeholder
        let command = format!("nmap {}", targets.join(" "));
        let args = options.unwrap_or_default();
        let task = RunCommandTask::new(command, args);

        self.add_task(task, Some(dependencies))
    }

    /// Add a Subfinder task to the workflow
    pub fn run_subfinder(
        &mut self,
        domain: String,
        options: Option<Vec<String>>,
        input_handle: Option<TaskHandle>,
    ) -> TaskHandle {
        // Implementar SubfinderTask cuando sea necesario
        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }

        // Por ahora, usamos RunCommandTask como un placeholder
        let command = format!("subfinder -d {}", domain);
        let args = options.unwrap_or_default();
        let task = RunCommandTask::new(command, args);

        self.add_task(task, Some(dependencies))
    }

    /// Add a Wappalyzer task to the workflow
    pub fn run_wappalyzer(&mut self, url: String, input_handle: Option<TaskHandle>) -> TaskHandle {
        // Implementar WappalyzerTask cuando sea necesario
        let mut dependencies = HashSet::new();
        if let Some(handle) = input_handle {
            dependencies.insert(handle);
        }

        // Por ahora, usamos RunCommandTask como un placeholder
        let command = format!("wappalyzer {}", url);
        let args = Vec::new();
        let task = RunCommandTask::new(command, args);

        self.add_task(task, Some(dependencies))
    }

    pub fn run_create_dir(&mut self, dir_path: String) -> TaskHandle {
        let dir_path = PathBuf::from(dir_path);
        let task = CreateDirTask::new(dir_path);

        self.add_task(task, None)
    }

    /// Get the output of a task
    pub fn get_output(&self, handle: TaskHandle) -> Option<TaskOutput> {
        self.tasks.get(&handle).map(|task| task.output.clone())
    }
}
