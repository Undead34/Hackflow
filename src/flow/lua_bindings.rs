use anyhow::Result;
use mlua::{Lua, Table, UserData, UserDataMethods, Value as LuaValue};

use super::core::Flow;
use super::task::TaskHandle;

impl UserData for Flow {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Generic command execution
        methods.add_method_mut(
            "run_command",
            |_, this, (command, args, input_handle): (String, Option<Vec<String>>, Option<TaskHandle>)| {
                let handle = this.run_command(command, args, input_handle);
                Ok(handle)
            },
        );

        // Print message
        methods.add_method_mut("print", |_, this, message: String| {
            let handle = this.print(message);
            Ok(handle)
        });

        // DNS lookup
        methods.add_method_mut("dns_lookup", |_lua, this, params: Table| {
            let domain: String = params.get("domain")?;
            let args: Option<Vec<String>> = params.get("args").unwrap_or(None);
            let replace_args: Option<bool> = params.get("replace_args").unwrap_or(None);

            let input_handle: Option<TaskHandle> = params.get("input_handle").unwrap_or(None);

            let handle = this.dns_lookup(domain, args, replace_args, input_handle);
            Ok(handle)
        });

        // Nmap scan
        methods.add_method_mut("run_nmap", |_lua, this, params: Table| {
            let targets: Vec<String> = match params.get("targets")? {
                LuaValue::String(s) => vec![s.to_str()?.to_string()],
                LuaValue::Table(t) => {
                    let mut result = Vec::new();
                    for pair in t.pairs::<i32, String>() {
                        let (_, value) = pair?;
                        result.push(value);
                    }
                    result
                }
                _ => {
                    return Err(mlua::Error::RuntimeError(
                        "targets must be a string or table".to_string(),
                    ))
                }
            };

            let options: Option<Vec<String>> = params.get("options").unwrap_or(None);
            let input_handle: Option<TaskHandle> = params.get("input_handle").unwrap_or(None);

            let handle = this.run_nmap(targets, options, input_handle);
            Ok(handle)
        });

        // Subfinder
        methods.add_method_mut("run_subfinder", |_lua, this, params: Table| {
            let domain: String = params.get("domain")?;
            let options: Option<Vec<String>> = params.get("options").unwrap_or(None);
            let input_handle: Option<TaskHandle> = params.get("input_handle").unwrap_or(None);

            let handle = this.run_subfinder(domain, options, input_handle);
            Ok(handle)
        });

        // Wappalyzer
        methods.add_method_mut("run_wappalyzer", |_lua, this, params: Table| {
            let url: String = params.get("url")?;
            let input_handle: Option<TaskHandle> = params.get("input_handle").unwrap_or(None);

            let handle = this.run_wappalyzer(url, input_handle);
            Ok(handle)
        });

        // Sequential execution
        methods.add_method_mut("execute", |_, this, ()| {
            let success = this.execute();
            Ok(success)
        });

        // Parallel execution
        methods.add_method_mut("execute_parallel", |_, this, ()| {
            let success = this.execute_parallel();
            Ok(success)
        });

        // Create dir
        methods.add_method_mut("create_dir", |_, this, dir_path| {
            let handle = this.run_create_dir(dir_path);
            Ok(handle)
        });

        // Export JSON
        methods.add_method("export_json", |_, this, params: Table| {
            let handle: TaskHandle = params.get("handle")?;
            let dir_path: String = params.get("dir_path")?;
            let filename: Option<String> = params.get("filename").unwrap_or(None);

            let success = this.export_json(handle, dir_path, filename);
            Ok(success)
        });

        // Export CSV
        methods.add_method_mut("export_csv", |_lua, this, params: Table| {
            let handle: TaskHandle = params.get("handle")?;
            let dir_path: String = params.get("dir_path")?;
            let filename: Option<String> = params.get("filename").unwrap_or(None);

            let handle = this.export_csv(handle, dir_path, filename);
            Ok(handle)
        });

        // Export Raw
        methods.add_method("export_raw", |_, this, params: Table| {
            let handle: TaskHandle = params.get("handle")?;
            let dir_path: String = params.get("dir_path")?;
            let filename: Option<String> = params.get("filename").unwrap_or(None);

            let success = this.export_raw(handle, dir_path, filename);
            Ok(success)
        });

        // Pretty print task output
        methods.add_method("pretty", |_, this, handle: TaskHandle| {
            this.pretty(handle);
            Ok(())
        });

        // Check task status and return detailed information
        methods.add_method("check_task_status", |lua, this, handle: TaskHandle| {
            let table = lua.create_table()?;

            if let Some(output) = this.get_output(handle) {
                let is_error = match &output {
                    super::task::TaskOutput::None => true,
                    super::task::TaskOutput::Json(json) => {
                        if let serde_json::Value::Object(obj) = json {
                            obj.contains_key("error") || obj.contains_key("exit_code")
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                table.set("success", !is_error)?;

                if is_error {
                    if let super::task::TaskOutput::Json(json) = &output {
                        if let serde_json::Value::Object(obj) = json {
                            // Extraer información de error
                            if let Some(serde_json::Value::String(error)) = obj.get("error") {
                                table.set("error", error.clone())?;
                            }

                            if let Some(serde_json::Value::Number(code)) = obj.get("exit_code") {
                                if let Some(code) = code.as_i64() {
                                    table.set("exit_code", code)?;
                                }
                            }

                            if let Some(serde_json::Value::String(stderr)) = obj.get("stderr") {
                                table.set("stderr", stderr.clone())?;
                            }

                            if let Some(serde_json::Value::String(stdout)) = obj.get("stdout") {
                                table.set("stdout", stdout.clone())?;
                            }
                        }
                    }
                }
            } else {
                table.set("success", false)?;
                table.set("error", "Task not found or not executed")?;
            }

            Ok(LuaValue::Table(table))
        });

        // Get task output
        methods.add_method("get_output", |lua, this, handle: TaskHandle| {
            if let Some(output) = this.get_output(handle) {
                match output {
                    super::task::TaskOutput::None => Ok(LuaValue::Nil),
                    super::task::TaskOutput::String(s) => {
                        Ok(LuaValue::String(lua.create_string(&s)?))
                    }
                    super::task::TaskOutput::Bool(s) => Ok(LuaValue::Boolean(s)),
                    super::task::TaskOutput::Json(json) => {
                        // Convert JSON to Lua table
                        let table = lua.create_table()?;
                        if let serde_json::Value::Object(obj) = json {
                            for (k, v) in obj {
                                match v {
                                    serde_json::Value::String(s) => table.set(k, s)?,
                                    serde_json::Value::Number(n) => {
                                        if let Some(i) = n.as_i64() {
                                            table.set(k, i)?
                                        } else if let Some(f) = n.as_f64() {
                                            table.set(k, f)?
                                        }
                                    }
                                    serde_json::Value::Bool(b) => table.set(k, b)?,
                                    serde_json::Value::Array(arr) => {
                                        let arr_table = lua.create_table()?;
                                        for (i, item) in arr.iter().enumerate() {
                                            if let serde_json::Value::String(s) = item {
                                                arr_table.set(i + 1, s.clone())?
                                            }
                                        }
                                        table.set(k, arr_table)?
                                    }
                                    _ => {} // Skip null and objects for simplicity
                                }
                            }
                        }
                        Ok(LuaValue::Table(table))
                    }
                    super::task::TaskOutput::Binary(_) => Ok(LuaValue::Nil), // Binary data not exposed to Lua
                    super::task::TaskOutput::DnsLookup {
                        csv_file,
                        json_file,
                        exit_code,
                        domain,
                        stderr,
                        stdout,
                        raw_stdout,
                        raw_stderr,
                    } => {
                        let table = lua.create_table()?;
                        let csv_path_str = csv_file.to_str().unwrap_or_default();
                        let json_path_str = json_file.to_str().unwrap_or_default();

                        table.set("csv_file", csv_path_str)?;
                        table.set("json_file", json_path_str)?;
                        table.set("exit_code", exit_code)?;
                        table.set("stderr", stderr)?;
                        table.set("stdout", stdout)?;
                        table.set("domain", domain)?;
                        table.set("raw_stdout", raw_stdout)?;
                        table.set("raw_stderr", raw_stderr)?;

                        Ok(LuaValue::Table(table))
                    }
                    super::task::TaskOutput::Subfinder(domains) => {
                        let table = lua.create_table()?;
                        for (i, domain) in domains.iter().enumerate() {
                            table.set(i + 1, domain.clone())?;
                        }
                        Ok(LuaValue::Table(table))
                    }
                    super::task::TaskOutput::Nmap(json) => {
                        // Convert JSON to Lua table (simplified)
                        let table = lua.create_table()?;
                        if let serde_json::Value::Object(obj) = json {
                            for (k, v) in obj {
                                if let serde_json::Value::Array(arr) = v {
                                    let arr_table = lua.create_table()?;
                                    for (i, item) in arr.iter().enumerate() {
                                        if let serde_json::Value::String(s) = item {
                                            arr_table.set(i + 1, s.clone())?
                                        }
                                    }
                                    table.set(k, arr_table)?
                                }
                            }
                        }
                        Ok(LuaValue::Table(table))
                    }
                }
            } else {
                Ok(LuaValue::Nil)
            }
        });
    }
}

/// Register the Flow module in Lua
pub fn register_module(lua: &Lua) -> Result<()> {
    let flow = Flow::new();
    let flow = lua.create_userdata(flow)?;
    lua.globals().set("flow", flow)?;

    Ok(())
}
