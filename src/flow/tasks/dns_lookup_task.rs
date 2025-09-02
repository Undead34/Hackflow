use crate::flow::task::{TaskHandle, TaskOutput};
use crate::flow::task_executor::TaskExecutor;
use crate::{define_task, impl_task_executor};

use std::env::temp_dir;
use std::path::PathBuf;
use wslpath_rs::{windows_to_wsl, wsl_to_windows};

use tokio::process::Command;

define_task!(DnsLookup, DnsLookupTask, domain: String, args: Option<Vec<String>>, replace_args: Option<bool>);

impl DnsLookupTask {
    pub fn new(domain: String, args: Option<Vec<String>>, replace_args: Option<bool>) -> Self {
        Self {
            domain,
            args,
            replace_args,
            input_handle: None,
        }
    }

    async fn execute_impl(&self) -> TaskOutput {
        let client_id = self.domain.replace('.', "-");
        let tool_name = "dnsrecon";
        let scan_type = "std";
        let date_tag = chrono::Local::now().format("%Y%m%d_%H%M").to_string();

        // Usar PathBuf para construir rutas de forma robusta
        let base_filename = format!("{}_{}_{}_{}", client_id, tool_name, scan_type, date_tag);

        // Asume que quieres los archivos en el directorio actual "."
        let current_dir = temp_dir();
        let mut csv_path = current_dir.join(format!("{}.csv", base_filename));
        let mut json_path = current_dir.join(format!("{}.json", base_filename));

        if cfg!(target_os = "windows") {
            csv_path =
                PathBuf::from(windows_to_wsl(csv_path.to_str().unwrap_or_default()).unwrap());
            json_path =
                PathBuf::from(windows_to_wsl(json_path.to_str().unwrap_or_default()).unwrap());
        }

        let mut command;

        if cfg!(target_os = "windows") {
            command = Command::new("wsl");
            command.args(["--distribution", "kali-linux", "--", "dnsrecon"]);
        } else {
            command = Command::new("dnsrecon");
        }

        if let Some(args) = self.args.clone() {
            command.args(args);
        }

        let replace = if let Some(replace) = self.replace_args {
            replace
        } else {
            false
        };

        if !replace {
            command
                .arg("-a")
                .arg("-t")
                .arg(scan_type)
                .arg("-d")
                .arg(&self.domain)
                .arg("-c")
                .arg(csv_path.clone())
                .arg("-j")
                .arg(json_path.clone());
        }

        if cfg!(target_os = "windows") {
            csv_path =
                PathBuf::from(wsl_to_windows(csv_path.to_str().unwrap_or_default()).unwrap());
            json_path =
                PathBuf::from(wsl_to_windows(json_path.to_str().unwrap_or_default()).unwrap());
        }

        let output = command
            .output()
            .await
            .map_err(|e| format!("Failed to spawn command: {}", e));

        match output {
            Ok(output) => {
                let exit_code = output.status.code().unwrap_or(0);
                // Preservar los códigos de escape ANSI para mantener los colores
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                // También guardar los bytes crudos como cadenas hexadecimales para preservar todo el contenido original
                let raw_stdout = output.stdout.clone();
                let raw_stderr = output.stderr.clone();

                if output.status.success() {
                    TaskOutput::DnsLookup {
                        csv_file: csv_path,
                        json_file: json_path,
                        exit_code,
                        domain: self.domain.clone(),
                        stdout,
                        stderr,
                        raw_stdout,
                        raw_stderr,
                    }
                } else {
                    // Crear un objeto JSON con la información del error
                    let mut error_info = serde_json::Map::new();
                    error_info.insert(
                        "exit_code".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(exit_code)),
                    );
                    error_info.insert("stdout".to_string(), serde_json::Value::String(stdout));
                    error_info.insert("stderr".to_string(), serde_json::Value::String(stderr));
                    error_info.insert(
                        "error".to_string(),
                        serde_json::Value::String(format!(
                            "Command failed with exit code {}",
                            exit_code
                        )),
                    );

                    TaskOutput::Json(serde_json::Value::Object(error_info))
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to execute command: {}", e);
                println!("TaskOutput: {}", error_msg);

                // Crear un objeto JSON con la información del error
                let mut error_info = serde_json::Map::new();
                error_info.insert("error".to_string(), serde_json::Value::String(error_msg));
                error_info.insert(
                    "exit_code".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(127)),
                ); // Código de error estándar para "command not found"

                TaskOutput::Json(serde_json::Value::Object(error_info))
            }
        }
    }

    fn process_input(&mut self, _input: &TaskOutput) -> Option<TaskOutput> {
        // DnsLookupTask actualmente no utiliza la salida de otras tareas,
        // pero mantenemos el método para compatibilidad futura.
        None
    }
}
