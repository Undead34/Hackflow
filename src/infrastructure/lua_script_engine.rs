use mlua::{Function, Lua, Table};

use crate::domain::{ExecutionContext, LuaScript, ScriptEnginePort};

#[derive(Clone, Default)]
pub struct LuaScriptEngine;

impl LuaScriptEngine {
    pub fn new() -> Self {
        Self
    }

    fn build_context_table(lua: &Lua, context: &ExecutionContext) -> mlua::Result<Table> {
        let ctx = lua.create_table()?;
        let vars = lua.create_table()?;
        for (key, value) in &context.variables {
            vars.set(key.as_str(), value.as_str())?;
        }
        ctx.set("variables", vars)?;

        let past_results = lua.create_table()?;
        for (task_id, output) in &context.past_results {
            let output_table = lua.create_table()?;
            output_table.set("stdout", output.stdout.as_str())?;
            output_table.set("stderr", output.stderr.as_str())?;
            output_table.set("exit_code", output.exit_code)?;
            past_results.set(task_id.as_str(), output_table)?;
        }
        ctx.set("past_results", past_results)?;

        Ok(ctx)
    }

    fn wrap_condition(script: &LuaScript) -> String {
        format!("return function(context)\n{}\nend", script.0)
    }

    fn wrap_transform(script: &LuaScript) -> String {
        format!("return function(raw, context)\n{}\nend", script.0)
    }
}

#[async_trait::async_trait]
impl ScriptEnginePort for LuaScriptEngine {
    async fn evaluate_condition(&self, script: &LuaScript, context: &ExecutionContext) -> bool {
        let lua = Lua::new();
        let ctx_table = match Self::build_context_table(&lua, context) {
            Ok(t) => t,
            Err(_) => return false,
        };

        let wrapped = Self::wrap_condition(script);
        let function: mlua::Result<Function> = lua.load(&wrapped).eval();

        match function {
            Ok(func) => func.call::<bool>(ctx_table).unwrap_or(false),
            Err(_) => false,
        }
    }

    async fn transform_output(
        &self,
        script: &LuaScript,
        raw_output: &str,
        context: &ExecutionContext,
    ) -> String {
        let lua = Lua::new();
        let ctx_table = match Self::build_context_table(&lua, context) {
            Ok(t) => t,
            Err(err) => {
                eprintln!("Failed to map context for Lua: {err}");
                return raw_output.to_string();
            }
        };

        let wrapped = Self::wrap_transform(script);
        let function: mlua::Result<Function> = lua.load(&wrapped).eval();

        match function {
            Ok(func) => func
                .call::<String>((raw_output.to_string(), ctx_table))
                .unwrap_or_else(|_| raw_output.to_string()),
            Err(err) => {
                eprintln!("Failed to compile Lua script: {err}");
                raw_output.to_string()
            }
        }
    }
}
