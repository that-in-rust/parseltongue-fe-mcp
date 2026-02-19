use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Verification tool '{tool}' not found")]
    ToolNotFound { tool: String },

    #[error("Failed to execute '{tool}': {source}")]
    ToolExecution {
        tool: String,
        source: std::io::Error,
    },

    #[error("Failed to parse output from '{tool}': {message}")]
    ParseError { tool: String, message: String },

    #[error("Verification timed out after {seconds}s")]
    Timeout { seconds: u64 },
}
