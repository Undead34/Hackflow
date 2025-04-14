use std::fs;
use std::path::Path;

use crate::flow::task::{TaskHandle, TaskOutput};
use crate::flow::task_executor::TaskExecutor;
use crate::{define_task, impl_task_executor};

define_task!(ExportCSV, ExportCSVTask, output: TaskOutput, dir_path: String, filename: Option<String>);

impl ExportCSVTask {
    pub fn new(output: TaskOutput, dir_path: String, filename: Option<String>) -> Self {
        Self {
            output,
            dir_path,
            filename,
            input_handle: None,
        }
    }

    pub fn export_csv(&self) -> anyhow::Result<()> {
        match self.output.clone() {
            TaskOutput::DnsLookup { csv_file, .. } => {
                let target_dir = Path::new(&self.dir_path);
                if !target_dir.exists() {
                    fs::create_dir_all(target_dir)?;
                }

                let source_filename = csv_file.file_name().unwrap_or_default();
                let target_filename = if let Some(name) = self.filename.clone() {
                    Path::new(&name).with_extension("csv")
                } else {
                    Path::new(source_filename).to_path_buf()
                };

                let target_path = target_dir.join(target_filename);

                fs::copy(&csv_file, &target_path)?;
                Ok(())
            }
            _ => Err(anyhow::anyhow!(
                "Export CSV not supported for this task type"
            )),
        }
    }

    async fn execute_impl(&self) -> TaskOutput {
        if self.export_csv().is_err() {
            return TaskOutput::String("Export CSV failed".to_string());
        }

        TaskOutput::None
    }
}
