use crate::flow::task::{TaskHandle, TaskOutput};
use crate::flow::task_executor::TaskExecutor;
use crate::{define_task, impl_task_executor};
use tokio::time::Duration;

define_task!(RunCommand, RunCommandTask, command: String, args: Option<Vec<String>>);

impl RunCommandTask {
    pub fn new(command: String, args: Vec<String>) -> Self {
        Self {
            command,
            args: Some(args),
            input_handle: None,
        }
    }
    
    async fn execute_impl(&self) -> TaskOutput {
        println!("Running command: {} {:?}", self.command, self.args);
        
        // En una implementación real, usaríamos tokio::process::Command
        // Aquí simulamos el trabajo con un delay
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Devolvemos un resultado simulado
        TaskOutput::String(format!("Output from command: {}", self.command))
    }
    
    fn process_input(&self, _input: &TaskOutput) -> Option<TaskOutput> {
        // Aquí podríamos procesar la entrada de otra tarea
        // Por ejemplo, si la entrada es una lista de dominios, podríamos
        // ejecutar el comando para cada dominio
        None
    }
}
