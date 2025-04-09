use std::path::PathBuf;

use crate::flow::task::{TaskHandle, TaskOutput};
use crate::flow::task_executor::TaskExecutor;
use crate::{define_task, impl_task_executor};

define_task!(CreateDir, CreateDirTask, dir_path: PathBuf);

impl CreateDirTask {
    pub fn new(dir_path: impl Into<PathBuf>) -> Self {
        Self {
            dir_path: dir_path.into(),
            input_handle: None,
        }
    }

    async fn execute_impl(&self) -> TaskOutput {
        if let Ok(_) = tokio::fs::create_dir_all(self.dir_path.clone()).await {
            TaskOutput::Bool(true)
        } else {
            TaskOutput::Bool(false)
        }
    }
}
