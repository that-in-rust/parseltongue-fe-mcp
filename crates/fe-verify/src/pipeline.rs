use crate::detection::{DetectedTools, LinterKind, TestRunnerKind, TypeCheckerKind};
use crate::error::VerifyError;
use crate::runners::VerificationRunner;
use crate::runners::{biome::BiomeRunner, eslint::ESLintRunner};
use crate::runners::{jest::JestRunner, vitest::VitestRunner};
use crate::runners::typescript::TypeScriptRunner;
use crate::types::{VerificationSummary, StepResult, TestStepResult};
use std::path::Path;

/// Cascading verification pipeline: lint → types → tests.
/// Early-terminates on error (no point running tests if types fail).
pub struct VerificationPipeline {
    linter: Option<Box<dyn VerificationRunner>>,
    type_checker: Option<Box<dyn VerificationRunner>>,
    test_runner: Option<Box<dyn VerificationRunner>>,
}

impl VerificationPipeline {
    pub fn from_detected(tools: DetectedTools) -> Self {
        let linter: Option<Box<dyn VerificationRunner>> = match tools.linter {
            Some(LinterKind::ESLint { bin }) => Some(Box::new(ESLintRunner::new(bin))),
            Some(LinterKind::Biome { bin }) => Some(Box::new(BiomeRunner::new(bin))),
            None => None,
        };

        let type_checker: Option<Box<dyn VerificationRunner>> = match tools.type_checker {
            Some(TypeCheckerKind::Tsc { bin }) => Some(Box::new(TypeScriptRunner::new(bin))),
            None => None,
        };

        let test_runner: Option<Box<dyn VerificationRunner>> = match tools.test_runner {
            Some(TestRunnerKind::Jest { bin }) => Some(Box::new(JestRunner::new(bin))),
            Some(TestRunnerKind::Vitest { bin }) => Some(Box::new(VitestRunner::new(bin))),
            None => None,
        };

        Self {
            linter,
            type_checker,
            test_runner,
        }
    }

    /// Create an empty pipeline (no tools detected). Useful for testing.
    pub fn empty() -> Self {
        Self {
            linter: None,
            type_checker: None,
            test_runner: None,
        }
    }

    /// Run the full verification pipeline on the given files.
    pub async fn run(
        &self,
        project_root: &Path,
        affected_files: &[&Path],
    ) -> Result<VerificationSummary, VerifyError> {
        let mut summary = VerificationSummary::default();

        // Step 1: Lint
        if let Some(linter) = &self.linter {
            let result = linter.run(project_root, affected_files).await?;
            if result.exit_code != 0 {
                summary.lint = StepResult {
                    status: "fail".to_string(),
                    error_count: 1,
                    warning_count: 0,
                    errors: Vec::new(), // TODO: parse JSON output
                };
                summary.types = StepResult::skipped("Skipped due to lint errors");
                summary.tests = TestStepResult::skipped("Skipped due to lint errors");
                return Ok(summary);
            }
            summary.lint = StepResult::pass();
        }

        // Step 2: TypeCheck
        if let Some(type_checker) = &self.type_checker {
            let result = type_checker.run(project_root, affected_files).await?;
            if result.exit_code != 0 {
                summary.types = StepResult {
                    status: "fail".to_string(),
                    error_count: 1,
                    warning_count: 0,
                    errors: Vec::new(), // TODO: parse tsc output
                };
                summary.tests = TestStepResult::skipped("Skipped due to type errors");
                return Ok(summary);
            }
            summary.types = StepResult::pass();
        }

        // Step 3: Tests
        if let Some(test_runner) = &self.test_runner {
            let result = test_runner.run(project_root, affected_files).await?;
            if result.exit_code != 0 {
                summary.tests = TestStepResult {
                    status: "fail".to_string(),
                    ran: 0,
                    passed: 0,
                    failed: 1,
                    failures: Vec::new(), // TODO: parse test output
                };
                return Ok(summary);
            }
            summary.tests = TestStepResult {
                status: "pass".to_string(),
                ran: 0,
                passed: 0,
                failed: 0,
                failures: Vec::new(),
            };
        }

        Ok(summary)
    }

    pub fn has_any_tools(&self) -> bool {
        self.linter.is_some() || self.type_checker.is_some() || self.test_runner.is_some()
    }
}
