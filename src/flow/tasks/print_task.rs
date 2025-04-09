use crate::flow::task::{TaskHandle, TaskOutput};
use crate::flow::task_executor::TaskExecutor;
use crate::{define_task, impl_task_executor};

define_task!(Print, PrintTask, message: String);

impl PrintTask {
    pub fn new(message: String) -> Self {
        Self {
            message,
            input_handle: None,
        }
    }
    
    async fn execute_impl(&self) -> TaskOutput {
        println!("{}", self.message);
        TaskOutput::None
    }
}
