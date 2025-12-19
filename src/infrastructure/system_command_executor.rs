use std::process::Stdio;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use which::which;

use crate::domain::{CommandExecutorPort, CommandOutput, ShellCommand};

#[derive(Clone, Default)]
pub struct SystemCommandExecutor {
    pub preferred_wsl_distro: Option<String>,
}

impl SystemCommandExecutor {
    pub fn new() -> Self {
        Self {
            preferred_wsl_distro: Some("kali-linux".to_string()),
        }
    }

    fn prepare_command(&self, cmd: &ShellCommand) -> Result<(Command, bool), String> {
        let mut command: Command;
        let mut used_wsl = false;

        if cfg!(target_os = "windows") {
            match which(&cmd.program) {
                Ok(binary_path) => {
                    command = Command::new(binary_path);
                }
                Err(_) => {
                    used_wsl = true;
                    let distro = self
                        .preferred_wsl_distro
                        .clone()
                        .unwrap_or_else(|| "kali-linux".to_string());

                    command = Command::new("wsl");
                    command.args(["--distribution", &distro, "--", &cmd.program]);
                }
            }
        } else {
            command = Command::new(&cmd.program);
        }

        command.args(&cmd.args);

        if let Some(dir) = &cmd.working_dir {
            command.current_dir(dir);
        }

        command.envs(&cmd.env_vars);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        Ok((command, used_wsl))
    }
}

#[async_trait::async_trait]
impl CommandExecutorPort for SystemCommandExecutor {
    async fn execute(
        &self,
        cmd: &ShellCommand,
        input: Option<&str>,
    ) -> Result<CommandOutput, String> {
        let (mut command, _used_wsl) = self.prepare_command(cmd)?;

        if input.is_some() {
            command.stdin(Stdio::piped());
        }

        let mut child = command
            .spawn()
            .map_err(|err| format!("Failed to spawn command '{}': {err}", cmd.program))?;

        if let Some(stdin_data) = input {
            if let Some(stdin) = child.stdin.take() {
                let mut writer = tokio::io::BufWriter::new(stdin);
                writer
                    .write_all(stdin_data.as_bytes())
                    .await
                    .map_err(|err| format!("Failed to write to stdin: {err}"))?;
            }
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|err| format!("Failed to wait for command output: {err}"))?;

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}
