use serde::Serialize;

/// Summary of all verification steps.
#[derive(Debug, Clone, Serialize, Default)]
pub struct VerificationSummary {
    pub status: String,
    pub lint: StepResult,
    pub types: StepResult,
    pub tests: TestStepResult,
    /// Hint for the LLM agent on what to do next.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl VerificationSummary {
    pub fn is_passing(&self) -> bool {
        self.lint.status != "fail" && self.types.status != "fail" && self.tests.status != "fail"
    }

    /// Finalize the summary: set top-level status and suggestion based on step results.
    pub fn finalize(&mut self) {
        if self.is_passing() {
            self.status = "pass".into();
        } else {
            self.status = "fail".into();
            self.suggestion =
                Some("Call fe_doctor with the errors above for structured fix suggestions".into());
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub status: String,
    pub error_count: usize,
    pub warning_count: usize,
    pub errors: Vec<DiagnosticItem>,
}

impl Default for StepResult {
    fn default() -> Self {
        Self {
            status: "skipped".to_string(),
            error_count: 0,
            warning_count: 0,
            errors: Vec::new(),
        }
    }
}

impl StepResult {
    pub fn pass() -> Self {
        Self {
            status: "pass".to_string(),
            ..Default::default()
        }
    }

    pub fn skipped(reason: &str) -> Self {
        Self {
            status: format!("skipped: {reason}"),
            ..Default::default()
        }
    }

    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TestStepResult {
    pub status: String,
    pub ran: usize,
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<TestFailure>,
}

impl Default for TestStepResult {
    fn default() -> Self {
        Self {
            status: "skipped".to_string(),
            ran: 0,
            passed: 0,
            failed: 0,
            failures: Vec::new(),
        }
    }
}

impl TestStepResult {
    pub fn skipped(reason: &str) -> Self {
        Self {
            status: format!("skipped: {reason}"),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticItem {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub rule: Option<String>,
    pub severity: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestFailure {
    pub test_name: String,
    pub file: String,
    pub message: String,
}
