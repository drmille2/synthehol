use serde::Deserialize;
use std::process::{Command, ExitStatus};
use tokio::time::Instant;
use tracing::instrument;

/// A monitor execution target, such as a script or binary that produces
/// some output and a 0 result code indicating success, and >0 for failure.
/// Command-line arguments are provided by a vec of strings, and environment
/// variables by a vec of (String, String) tuples
#[derive(Debug)]
pub struct Target {
    pub path: String,
    pub args: Option<Vec<String>>,
    pub env: Option<Vec<(String, String)>>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct TargetArgs {
    pub path: String,
    pub args: Option<Vec<String>>,
    pub env: Option<Vec<(String, String)>>,
}

#[derive(Debug)]
pub struct TargetOutput {
    pub stdout: String,
    pub stderr: String,
    pub duration: u64,
    pub status: ExitStatus,
}

impl TargetArgs {
    pub fn build(self) -> Target {
        Target {
            path: self.path,
            args: self.args,
            env: self.env,
        }
    }
}

impl Target {
    /// Run the target, returning duration and other execution details
    #[instrument(level=tracing::Level::DEBUG)]
    pub fn run(&self) -> Result<TargetOutput, String> {
        let start = Instant::now();
        let mut cmd = Command::new(&self.path);
        if let Some(env) = self.env.clone() {
            cmd.envs(env);
        }
        if let Some(args) = self.args.clone() {
            cmd.args(args);
        }
        let output = cmd
            .output()
            .map_err(|e| format!("failed to run target ({0})", e))?;
        let stop = Instant::now();
        let duration = (stop - start).as_micros() as u64;
        let status = output.status;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let out = TargetOutput {
            stdout,
            stderr,
            duration,
            status,
        };
        Ok(out)
    }
}
