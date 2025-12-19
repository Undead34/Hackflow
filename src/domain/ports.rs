use super::values::{CommandOutput, ExecutionContext, LuaScript, ShellCommand};

#[async_trait::async_trait]
pub trait CommandExecutorPort: Send + Sync {
    async fn execute(
        &self,
        cmd: &ShellCommand,
        input: Option<&str>,
    ) -> Result<CommandOutput, String>;
}

#[async_trait::async_trait]
pub trait ScriptEnginePort: Send + Sync {
    async fn evaluate_condition(&self, script: &LuaScript, context: &ExecutionContext) -> bool;
    async fn transform_output(
        &self,
        script: &LuaScript,
        raw_output: &str,
        context: &ExecutionContext,
    ) -> String;
}
