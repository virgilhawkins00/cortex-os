use anyhow::Result;
use std::time::Duration;
use tokio::process::Command;
use tracing::warn;

/// Sandboxed execution environment for untrusted commands.
pub struct Sandbox {
    timeout: Duration,
    max_output_bytes: usize,
}

impl Default for Sandbox {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_output_bytes: 1024 * 512, // 512KB
        }
    }
}

impl Sandbox {
    #[must_use]
    pub fn new(timeout_secs: u64, max_output_bytes: usize) -> Self {
        Self {
            timeout: Duration::from_secs(timeout_secs),
            max_output_bytes,
        }
    }

    /// Execute a bash command with timeout and output limits.
    pub async fn exec_bash(&self, cmd: &str) -> Result<SandboxOutput> {
        let result = tokio::time::timeout(self.timeout, async {
            Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .await
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

                // Truncate if too large
                if stdout.len() > self.max_output_bytes {
                    warn!("stdout truncated from {} bytes", stdout.len());
                    stdout.truncate(self.max_output_bytes);
                    stdout.push_str("\n[OUTPUT TRUNCATED]");
                }
                if stderr.len() > self.max_output_bytes {
                    stderr.truncate(self.max_output_bytes);
                }

                Ok(SandboxOutput {
                    exit_code: output.status.code().unwrap_or(-1),
                    stdout,
                    stderr,
                    timed_out: false,
                })
            }
            Ok(Err(e)) => anyhow::bail!("Failed to spawn process: {e}"),
            Err(_) => Ok(SandboxOutput {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Command timed out after {}s", self.timeout.as_secs()),
                timed_out: true,
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SandboxOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}
