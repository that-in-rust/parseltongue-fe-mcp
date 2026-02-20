use crate::types::{TestFailure, TestStepResult};
use serde::Deserialize;

/// Vitest JSON reporter output (from `vitest run --reporter json`).
/// Format is similar to Jest but has some differences in structure.
#[derive(Deserialize)]
struct VitestOutput {
    #[serde(rename = "numTotalTests")]
    num_total_tests: usize,
    #[serde(rename = "numPassedTests")]
    num_passed_tests: usize,
    #[serde(rename = "numFailedTests")]
    num_failed_tests: usize,
    #[serde(rename = "testResults")]
    test_results: Vec<VitestTestResult>,
}

#[derive(Deserialize)]
struct VitestTestResult {
    name: String,
    #[serde(rename = "assertionResults")]
    assertion_results: Option<Vec<VitestAssertion>>,
}

#[derive(Deserialize)]
struct VitestAssertion {
    #[serde(rename = "ancestorTitles", default)]
    ancestor_titles: Vec<String>,
    title: String,
    status: String,
    #[serde(rename = "failureMessages", default)]
    failure_messages: Vec<String>,
}

pub fn parse_vitest_output(stdout: &str) -> TestStepResult {
    let json_start = match stdout.find('{') {
        Some(i) => i,
        None => {
            return TestStepResult {
                status: "fail".into(),
                ran: 0,
                passed: 0,
                failed: 1,
                failures: vec![TestFailure {
                    test_name: "<parse error>".into(),
                    file: String::new(),
                    message: "No JSON found in Vitest output".into(),
                }],
            };
        }
    };

    let output: VitestOutput = match serde_json::from_str(&stdout[json_start..]) {
        Ok(o) => o,
        Err(e) => {
            return TestStepResult {
                status: "fail".into(),
                ran: 0,
                passed: 0,
                failed: 1,
                failures: vec![TestFailure {
                    test_name: "<parse error>".into(),
                    file: String::new(),
                    message: format!("Failed to parse Vitest JSON: {e}"),
                }],
            };
        }
    };

    let mut failures = Vec::new();

    for suite in &output.test_results {
        if let Some(assertions) = &suite.assertion_results {
            for assertion in assertions {
                if assertion.status == "failed" {
                    let test_name = if assertion.ancestor_titles.is_empty() {
                        assertion.title.clone()
                    } else {
                        format!(
                            "{} > {}",
                            assertion.ancestor_titles.join(" > "),
                            assertion.title
                        )
                    };

                    failures.push(TestFailure {
                        test_name,
                        file: suite.name.clone(),
                        message: assertion
                            .failure_messages
                            .first()
                            .cloned()
                            .unwrap_or_default(),
                    });
                }
            }
        }
    }

    let status = if output.num_failed_tests > 0 {
        "fail"
    } else {
        "pass"
    };

    TestStepResult {
        status: status.into(),
        ran: output.num_total_tests,
        passed: output.num_passed_tests,
        failed: output.num_failed_tests,
        failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_passing() {
        let json = r#"{"numTotalTests":3,"numPassedTests":3,"numFailedTests":0,"testResults":[{"name":"src/App.test.tsx","assertionResults":[{"ancestorTitles":[],"title":"works","status":"passed","failureMessages":[]}]}]}"#;
        let result = parse_vitest_output(json);
        assert_eq!(result.status, "pass");
        assert_eq!(result.ran, 3);
    }

    #[test]
    fn test_parse_failures() {
        let json = r#"{
            "numTotalTests": 2,
            "numPassedTests": 0,
            "numFailedTests": 2,
            "testResults": [{
                "name": "src/utils.test.ts",
                "assertionResults": [
                    {"ancestorTitles": ["formatDate"], "title": "formats ISO dates", "status": "failed", "failureMessages": ["AssertionError: expected '2024' to be '2025'"]}
                ]
            }]
        }"#;
        let result = parse_vitest_output(json);
        assert_eq!(result.status, "fail");
        assert_eq!(result.failures.len(), 1);
        assert_eq!(
            result.failures[0].test_name,
            "formatDate > formats ISO dates"
        );
    }
}
