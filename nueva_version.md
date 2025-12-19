Tu diseño inicial es un excelente punto de partida, pero como intuiste, se queda "corto" porque mezcla **Definición** (qué hacer) con **Ejecución** (hacerlo), y le faltan los **Puertos** para desacoplarse de la infraestructura (Linux/Lua) y permitir el testeo.

Para llevar esto a un **Nivel Senior DDD con Arquitectura Hexagonal y Concurrencia**, necesitamos separar las preocupaciones en tres capas claras:

1. **Dominio (Core):** Lógica pura, structs, traits (puertos). Nada de `tokio` ni `std::process` aquí.
2. **Aplicación (Orquestación):** Manejo de actores/async y flujo de control.
3. **Infraestructura (Adaptadores):** Implementación real de comandos Linux y Lua.

Aquí tienes la evolución de tus structs para soportar este sistema complejo.

---

### 1. El Dominio: Contexto de Ejecución y Pipes

El problema principal en tu diseño anterior es que faltaba el "fluido" que viaja por los tubos (Pipes). Necesitamos un `ExecutionContext` que actúe como la memoria compartida del Workflow.

```rust
// domain/values.rs
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

// El "Contexto" es lo que viaja entre nodos. 
// Es lo que Lua leerá y modificará.
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    // Resultados de tareas previas: TaskId -> Output
    pub past_results: HashMap<TaskId, CommandOutput>,
    // Variables globales o de entorno dinámicas
    pub variables: HashMap<String, String>,
}

// Value Object para definir CÓMO pasar datos de A a B
#[derive(Debug, Clone)]
pub enum PipeStrategy {
    // La salida de la tarea anterior se inyecta como STDIN
    StdInFrom(TaskId), 
    // La salida se inyecta como una variable de entorno específica
    EnvVarFrom { source: TaskId, var_name: String },
    // La salida se pasa como argumento en la posición X
    ArgPositionFrom { source: TaskId, position: usize },
}

```

### 2. Los Puertos (La clave Hexagonal)

En DDD puro, tu dominio no debe saber *cómo* ejecutar un comando de Linux ni *cómo* interpretar Lua. Solo sabe que necesita hacerlo. Definimos Traits (Interfaces).

```rust
// domain/ports.rs

// Puerto para ejecutar comandos del SO.
// Usamos `async_trait` porque la IO es asíncrona.
#[async_trait::async_trait]
pub trait CommandExecutorPort: Send + Sync {
    async fn execute(&self, cmd: &ShellCommand, input: Option<&str>) -> Result<CommandOutput, String>;
}

// Puerto para el motor de Scripting (Lua).
// Recibe el contexto, corre el script, y devuelve una decisión o transformación.
#[async_trait::async_trait]
pub trait ScriptEnginePort: Send + Sync {
    async fn evaluate_condition(&self, script: &LuaScript, context: &ExecutionContext) -> bool;
    async fn transform_output(&self, script: &LuaScript, raw_output: &str) -> String;
}

```

### 3. Entidades Enriquecidas

Ahora tu `Task` debe ser consciente de los `Pipes`.

```rust
// domain/aggregates.rs

#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub command_def: ShellCommand,
    
    // Configuración de Entrada: ¿De dónde saco mis datos?
    pub input_pipes: Vec<PipeStrategy>,

    // Lógica de Lua
    pub pre_hook: Option<LuaScript>,  // Ej: ¿Debo correr?
    pub post_hook: Option<LuaScript>, // Ej: Parsear JSON de stdout
    
    // Estado interno (Value Object cambiado de tu versión)
    pub status: TaskStatus, 
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    ReadyToRun, // Sus dependencias acabaron
    Running,
    Completed(CommandOutput),
    Failed(String),
    Skipped(String), // Razón del skip
}

// Lógica de Dominio Puro
impl Task {
    /// Determina si la tarea puede ejecutarse basándose solo en el grafo
    pub fn try_transition_to_ready(&mut self, dependencies_met: bool) {
        if self.status == TaskStatus::Pending && dependencies_met {
            self.status = TaskStatus::ReadyToRun;
        }
    }
    
    /// Resuelve los argumentos finales combinando la definición estática
    /// con los datos dinámicos del contexto (Pipes).
    pub fn resolve_execution_params(&self, context: &ExecutionContext) -> (ShellCommand, Option<String>) {
        let mut final_cmd = self.command_def.clone();
        let mut stdin_buffer = None;

        for pipe in &self.input_pipes {
            match pipe {
                PipeStrategy::StdInFrom(source_id) => {
                    if let Some(output) = context.past_results.get(source_id) {
                        stdin_buffer = Some(output.stdout.clone());
                    }
                },
                PipeStrategy::EnvVarFrom { source, var_name } => {
                     if let Some(output) = context.past_results.get(source) {
                        final_cmd.env_vars.insert(var_name.clone(), output.stdout.clone());
                    }
                },
                // ... lógica para argumentos
                _ => {}
            }
        }
        (final_cmd, stdin_buffer)
    }
}

```

### 4. El Agregado y el Modelo de Actores (Async)

Aquí es donde entra el **Actor Model** (o su equivalente en Rust usando Tokio Channels). El `WorkflowAggregate` maneja el grafo, pero necesitamos un **WorkflowRunner** (Application Service) que orqueste la concurrencia.

Imagina que el `Workflow` es la base de datos en memoria, y el `WorkflowActor` es el trabajador.

```rust
// application/workflow_runner.rs
use tokio::sync::mpsc;

// Mensajes que el Actor del Workflow puede procesar
enum WorkflowMsg {
    Start,
    TaskCompleted { task_id: TaskId, result: CommandOutput },
    TaskFailed { task_id: TaskId, error: String },
}

pub struct WorkflowActor {
    // El estado del dominio
    workflow: Workflow,
    // Contexto acumulado
    execution_context: ExecutionContext,
    
    // Adaptadores (Inyectados)
    cmd_executor: Box<dyn CommandExecutorPort>,
    lua_engine: Box<dyn ScriptEnginePort>,
    
    // Canal para recibir eventos propios (Self-healing loop)
    receiver: mpsc::Receiver<WorkflowMsg>,
    sender: mpsc::Sender<WorkflowMsg>, // Para enviarse mensajes a sí mismo
}

impl WorkflowActor {
    pub async fn run(&mut self) {
        // Loop principal del Actor
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                WorkflowMsg::Start => self.schedule_next_tasks().await,
                WorkflowMsg::TaskCompleted { task_id, result } => {
                    self.handle_completion(task_id, result).await;
                    self.schedule_next_tasks().await; // Recursión asíncrona
                },
                WorkflowMsg::TaskFailed { .. } => {
                    // Lógica de reintentos o fallo total
                }
            }
        }
    }

    // Esta función busca en el grafo (DAG) tareas en estado 'Pending'
    // cuyas dependencias estén completas.
    async fn schedule_next_tasks(&mut self) {
        // 1. Preguntar al dominio qué tareas son elegibles
        let ready_tasks = self.workflow.get_executable_tasks();

        for task_id in ready_tasks {
            // 2. Marcar como Running en el dominio
            let task = self.workflow.get_mut_task(&task_id).unwrap();
            task.status = TaskStatus::Running;

            // 3. Clonar lo necesario para moverlo al hilo async (spawn)
            let task_clone = task.clone();
            let context_clone = self.execution_context.clone();
            let executor = self.cmd_executor.clone(); // Debe ser cloneable o Arc
            let script_engine = self.lua_engine.clone();
            let sender = self.sender.clone();

            // 4. CONCURRENCIA: Lanzar "Green Thread"
            tokio::spawn(async move {
                // A. Check Lua Pre-Hook
                if let Some(script) = &task_clone.pre_hook {
                    if !script_engine.evaluate_condition(script, &context_clone).await {
                         // Skip logic...
                         return;
                    }
                }

                // B. Resolver Pipes
                let (cmd, stdin) = task_clone.resolve_execution_params(&context_clone);

                // C. Ejecutar en el SO (Invocando adaptador)
                match executor.execute(&cmd, stdin.as_deref()).await {
                    Ok(output) => {
                        // D. Lua Post-Hook (Transformación)
                        let final_output = if let Some(transformer) = &task_clone.post_hook {
                             let transformed = script_engine.transform_output(transformer, &output.stdout).await;
                             CommandOutput { stdout: transformed, ..output }
                        } else {
                             output
                        };
                        
                        // E. Avisar al Actor principal
                        let _ = sender.send(WorkflowMsg::TaskCompleted { 
                            task_id: task_clone.id, 
                            result: final_output 
                        }).await;
                    },
                    Err(e) => {
                         let _ = sender.send(WorkflowMsg::TaskFailed { 
                            task_id: task_clone.id, 
                            error: e 
                        }).await;
                    }
                }
            });
        }
    }
}

```

### Resumen de Mejoras DDD + Hexagonal

1. **Contexto Explícito (`ExecutionContext`):** El flujo de datos ya no es mágico, es una estructura tangible que se pasa a los puertos.
2. **Inversión de Dependencias (Puertos):** `Task` no ejecuta nada. `WorkflowActor` usa `CommandExecutorPort`. Esto permite que en tus tests unitarios uses un `MockCommandExecutor` que simule salidas de Linux sin tocar la terminal real.
3. **Ejecución Asíncrona (Actor Model):** El `WorkflowActor` es el único que muta el estado del `Workflow`. Las tareas individuales se lanzan con `tokio::spawn`, pero reportan su resultado de vuelta al canal del Actor (`mpsc`), garantizando que no haya *Race Conditions* en el estado del grafo.
4. **Pipes como Estrategias:** `PipeStrategy` define declarativamente cómo conectar la salida de A con la entrada de B, permitiendo que el dominio resuelva esa complejidad antes de ejecutar.

¿Te gustaría que profundice en cómo implementar el **Adaptador de Lua** (`ScriptEnginePort`) usando `mlua` para que pueda leer el `ExecutionContext` de Rust de forma segura?