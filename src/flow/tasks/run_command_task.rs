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

    fn process_input(&mut self, input: &TaskOutput) -> Option<TaskOutput> {
        // Permite añadir argumentos dinámicamente en función de la salida
        // de tareas previas.
        match input {
            TaskOutput::String(s) => {
                // Usamos la cadena como argumento adicional
                self.args.get_or_insert_with(Vec::new).push(s.clone());
                None
            }
            TaskOutput::Subfinder(domains) => {
                // Añadir todos los dominios como argumentos
                self.args
                    .get_or_insert_with(Vec::new)
                    .extend(domains.clone());
                None
            }
            TaskOutput::DnsLookup { domain, .. } => {
                // Añadir el dominio resuelto como argumento
                self.args.get_or_insert_with(Vec::new).push(domain.clone());
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appends_string_to_args() {
        let mut task = RunCommandTask::new("cmd".into(), vec!["base".into()]);
        task.process_input(&TaskOutput::String("extra".into()));
        assert_eq!(
            task.args.unwrap(),
            vec!["base".to_string(), "extra".to_string()]
        );
    }

    #[test]
    fn extends_with_subfinder_domains() {
        let mut task = RunCommandTask::new("cmd".into(), vec![]);
        let domains = vec!["a.com".to_string(), "b.com".to_string()];
        task.process_input(&TaskOutput::Subfinder(domains.clone()));
        assert_eq!(task.args.unwrap(), domains);
    }
}
