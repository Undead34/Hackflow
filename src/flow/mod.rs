mod core;
mod lua_bindings;
mod task;
mod task_executor;
mod tasks;

use anyhow::{Context, Result};
use mlua::{Function as LuaFunction, Lua, Table as LuaTable};
use std::fs;
use std::path::PathBuf;

/// Represents the state of a workflow execution
pub enum WorkflowState {
    /// Workflow is ready to be executed
    Ready,
    /// Workflow is currently running
    Running,
    /// Workflow has completed successfully
    Completed,
    /// Workflow has failed
    Failed(String),
}

/// Represents a workflow that can be executed
pub struct Workflow {
    /// Path to the Lua workflow file
    file_path: PathBuf,
    /// Lua context
    lua: Lua,
    /// Current state of the workflow
    state: WorkflowState,
}

impl Workflow {
    /// Create a new workflow from a Lua file
    pub fn new(file_path: impl Into<PathBuf>) -> Result<Self> {
        let file_path = file_path.into();

        // Verify the file exists
        if !file_path.exists() {
            return Err(anyhow::anyhow!(
                "Workflow file not found: {}",
                file_path.display()
            ));
        }

        // Create Lua context
        let lua = Lua::new();

        // Register our flow module
        lua_bindings::register_module(&lua)?;

        // Load the workflow file
        let lua_file = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read workflow file: {}", file_path.display()))?;

        if lua_file.is_empty() {
            return Err(anyhow::anyhow!("Workflow file is empty"));
        }

        // Extract metadata
        Ok(Self {
            file_path,
            lua,
            state: WorkflowState::Ready,
        })
    }

    /// Get the current state of the workflow
    pub fn state(&self) -> &WorkflowState {
        &self.state
    }

    /// Get a reference to the Lua context
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Get a mutable reference to the Lua context
    pub fn lua_mut(&mut self) -> &mut Lua {
        &mut self.lua
    }

    /// Execute the workflow
    pub fn execute(&mut self, args: Vec<String>) -> Result<bool> {
        self.state = WorkflowState::Running;

        let lua_file = fs::read_to_string(&self.file_path).with_context(|| {
            format!("Failed to read workflow file: {}", self.file_path.display())
        })?;

        let workflow_table: LuaTable = self
            .lua
            .load(&lua_file)
            .eval()
            .context("Failed to evaluate Lua workflow file")?;

        let main_fn: LuaFunction = workflow_table
            .get("main")
            .context("Workflow must have a 'main' function")?;

        let result = match main_fn.call(args) {
            Ok(success) => {
                if success {
                    self.state = WorkflowState::Completed;
                } else {
                    self.state = WorkflowState::Failed("Workflow returned false".to_string());
                }
                Ok(success)
            }
            Err(err) => {
                let error_msg = format!("Error executing workflow: {}", err);
                self.state = WorkflowState::Failed(error_msg.clone());
                Err(anyhow::anyhow!(error_msg))
            }
        };

        result
    }
}
