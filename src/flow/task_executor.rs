use std::future::Future;
use std::pin::Pin;

use super::task::{TaskHandle, TaskOutput};

/// Trait que define la interfaz para ejecutores de tareas
pub trait TaskExecutor: Send + Sync {
    /// Nombre de la tarea para logging
    fn name(&self) -> &str;

    /// Ejecuta la tarea de forma asíncrona
    /// Usamos Pin<Box<dyn Future>> para hacer que el trait sea compatible con objetos
    fn execute<'a>(&'a self) -> Pin<Box<dyn Future<Output = TaskOutput> + Send + 'a>>;

    /// Obtiene el handle de entrada (si existe)
    fn input_handle(&self) -> Option<TaskHandle>;

    /// Convierte la salida de la tarea de entrada en un formato utilizable por esta tarea
    ///
    /// Se pasa un `&mut self` para permitir que la tarea actualice su estado
    /// interno en función de la salida recibida.
    fn process_input(&mut self, _input: &TaskOutput) -> Option<TaskOutput> {
        None // Por defecto, no procesa la entrada
    }

    /// Crea una representación en string de la tarea
    fn to_string(&self) -> String {
        format!("{}", self.name())
    }

    /// Clona el ejecutor de tareas en un nuevo Box
    fn clone_box(&self) -> Box<dyn TaskExecutor + Send + Sync>;
}

/// Macro para implementar TaskExecutor para una struct
#[macro_export]
macro_rules! impl_task_executor {
    ($type:ty, $name:expr) => {
        impl TaskExecutor for $type {
            fn name(&self) -> &str {
                $name
            }

            fn execute<'a>(&'a self) -> Pin<Box<dyn Future<Output = TaskOutput> + Send + 'a>> {
                Box::pin(self.execute_impl())
            }

            fn input_handle(&self) -> Option<TaskHandle> {
                self.input_handle
            }

            fn to_string(&self) -> String {
                format!("{}: {:?}", self.name(), self)
            }

            fn clone_box(&self) -> Box<dyn TaskExecutor + Send + Sync> {
                Box::new(self.clone())
            }
        }
    };
}

/// Macro para definir una nueva tarea
#[macro_export]
macro_rules! define_task {
    ($name:ident, $struct_name:ident, $($field:ident: $type:ty),*) => {
        use std::pin::Pin;
        use std::future::Future;

        #[derive(Clone, Debug)]
        pub struct $struct_name {
            $(pub $field: $type,)*
            pub input_handle: Option<TaskHandle>,
        }

        impl_task_executor!($struct_name, stringify!($name));
    };
}
