use super::{RunnerOutput, VerificationRunner};
use crate::error::VerifyError;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub struct TypeScriptRunner {
    bin: PathBuf,
}

impl TypeScriptRunner {
    pub fn new(bin: PathBuf) -> Self {
        Self { bin }
    }
}

#[async_trait::async_trait]
impl VerificationRunner for TypeScriptRunner {
    fn name(&self) -> &str {
        "tsc"
    }

    async fn run(
        &self,
        project_root: &Path,
        _files: &[&Path],
    ) -> Result<RunnerOutput, VerifyError> {
        let mut cmd = Command::new(&self.bin);
        cmd.current_dir(project_root);
        cmd.args(["--noEmit", "--pretty", "false"]);

        let output = cmd.output().await.map_err(|e| VerifyError::ToolExecution {
            tool: "tsc".into(),
            source: e,
        })?;

        Ok(RunnerOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}
