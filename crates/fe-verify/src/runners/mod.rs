pub mod eslint;
pub mod biome;
pub mod typescript;
pub mod jest;
pub mod vitest;

use crate::error::VerifyError;
use std::path::Path;

/// Output from a verification runner before parsing into specific types.
#[derive(Debug)]
pub struct RunnerOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Trait for all verification tool runners.
#[async_trait::async_trait]
pub trait VerificationRunner: Send + Sync {
    fn name(&self) -> &str;

    async fn run(
        &self,
        project_root: &Path,
        files: &[&Path],
    ) -> Result<RunnerOutput, VerifyError>;
}
