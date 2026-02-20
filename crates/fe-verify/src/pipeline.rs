use crate::detection::{DetectedTools, LinterKind, TestRunnerKind, TypeCheckerKind};
use crate::error::VerifyError;
use crate::parsers;
use crate::runners::VerificationRunner;
use crate::runners::{biome::BiomeRunner, eslint::ESLintRunner};
use crate::runners::{jest::JestRunner, vitest::VitestRunner};
use crate::runners::typescript::TypeScriptRunner;
use crate::types::{StepResult, TestStepResult, VerificationSummary};
use std::path::Path;

/// Which linter is active (needed to pick the right parser).
#[derive(Debug, Clone, Copy)]
enum ActiveLinter {
    ESLint,
    Biome,
}

/// Which test runner is active.
#[derive(Debug, Clone, Copy)]
enum ActiveTestRunner {
    Jest,
    Vitest,
}

/// Cascading verification pipeline: lint → types → tests.
/// Early-terminates on error (no point running tests if types fail).
pub struct VerificationPipeline {
    linter: Option<Box<dyn VerificationRunner>>,
    active_linter: Option<ActiveLinter>,
    type_checker: Option<Box<dyn VerificationRunner>>,
    test_runner: Option<Box<dyn VerificationRunner>>,
    active_test_runner: Option<ActiveTestRunner>,
}

impl VerificationPipeline {
    pub fn from_detected(tools: DetectedTools) -> Self {
        let (linter, active_linter): (Option<Box<dyn VerificationRunner>>, _) = match tools.linter
        {
            Some(LinterKind::ESLint { bin }) => {
                (Some(Box::new(ESLintRunner::new(bin))), Some(ActiveLinter::ESLint))
            }
            Some(LinterKind::Biome { bin }) => {
                (Some(Box::new(BiomeRunner::new(bin))), Some(ActiveLinter::Biome))
            }
            None => (None, None),
        };

        let type_checker: Option<Box<dyn VerificationRunner>> = match tools.type_checker {
            Some(TypeCheckerKind::Tsc { bin }) => Some(Box::new(TypeScriptRunner::new(bin))),
            None => None,
        };

        let (test_runner, active_test_runner): (Option<Box<dyn VerificationRunner>>, _) =
            match tools.test_runner {
                Some(TestRunnerKind::Jest { bin }) => {
                    (Some(Box::new(JestRunner::new(bin))), Some(ActiveTestRunner::Jest))
                }
                Some(TestRunnerKind::Vitest { bin }) => {
                    (Some(Box::new(VitestRunner::new(bin))), Some(ActiveTestRunner::Vitest))
                }
                None => (None, None),
            };

        Self {
            linter,
            active_linter,
            type_checker,
            test_runner,
            active_test_runner,
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
            let output = linter.run(project_root, affected_files).await?;
            let step = match self.active_linter {
                Some(ActiveLinter::ESLint) => parsers::eslint::parse_eslint_output(&output.stdout),
                Some(ActiveLinter::Biome) => {
                    // Biome JSON uses the same shape as ESLint for our purposes
                    parsers::eslint::parse_eslint_output(&output.stdout)
                }
                None => StepResult::pass(),
            };
            let failed = step.status == "fail";
            summary.lint = step;

            if failed {
                summary.types = StepResult::skipped("Skipped due to lint errors");
                summary.tests = TestStepResult::skipped("Skipped due to lint errors");
                summary.finalize();
                return Ok(summary);
            }
        }

        // Step 2: TypeCheck
        if let Some(type_checker) = &self.type_checker {
            let output = type_checker.run(project_root, affected_files).await?;
            let step = parsers::typescript::parse_tsc_output(&output.stdout);
            let failed = step.status == "fail";
            summary.types = step;

            if failed {
                summary.tests = TestStepResult::skipped("Skipped due to type errors");
                summary.finalize();
                return Ok(summary);
            }
        }

        // Step 3: Tests
        if let Some(test_runner) = &self.test_runner {
            let output = test_runner.run(project_root, affected_files).await?;
            summary.tests = match self.active_test_runner {
                Some(ActiveTestRunner::Jest) => parsers::jest::parse_jest_output(&output.stdout),
                Some(ActiveTestRunner::Vitest) => {
                    parsers::vitest::parse_vitest_output(&output.stdout)
                }
                None => TestStepResult::default(),
            };
        }

        summary.finalize();
        Ok(summary)
    }

    pub fn has_any_tools(&self) -> bool {
        self.linter.is_some() || self.type_checker.is_some() || self.test_runner.is_some()
    }
}
