use crate::types::TestStepResult;
use crate::types::TestFailure;
use serde::Deserialize;

/// Raw Jest JSON output (from `jest --json`).
#[derive(Deserialize)]
struct JestOutput {
    #[serde(rename = "numTotalTests")]
    num_total_tests: usize,
    #[serde(rename = "numPassedTests")]
    num_passed_tests: usize,
    #[serde(rename = "numFailedTests")]
    num_failed_tests: usize,
    #[serde(rename = "testResults")]
    test_results: Vec<JestTestResult>,
}

#[derive(Deserialize)]
struct JestTestResult {
    name: String,
    #[serde(rename = "assertionResults")]
    assertion_results: Option<Vec<JestAssertion>>,
}

#[derive(Deserialize)]
struct JestAssertion {
    #[serde(rename = "ancestorTitles")]
    ancestor_titles: Vec<String>,
    title: String,
    status: String,
    #[serde(rename = "failureMessages", default)]
    failure_messages: Vec<String>,
}

pub fn parse_jest_output(stdout: &str) -> TestStepResult {
    // Jest sometimes prefixes JSON with non-JSON text (e.g., console.log output).
    // Find the first '{' to locate the JSON start.
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
                    message: "No JSON found in Jest output".into(),
                }],
            };
        }
    };

    let output: JestOutput = match serde_json::from_str(&stdout[json_start..]) {
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
                    message: format!("Failed to parse Jest JSON: {e}"),
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
                        format!("{} > {}", assertion.ancestor_titles.join(" > "), assertion.title)
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

    let status = if output.num_failed_tests > 0 { "fail" } else { "pass" };

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
    fn test_parse_all_passing() {
        let json = r#"{"numTotalTests":5,"numPassedTests":5,"numFailedTests":0,"testResults":[{"name":"/src/__tests__/App.test.tsx","assertionResults":[{"ancestorTitles":["App"],"title":"renders","status":"passed","failureMessages":[]}]}]}"#;
        let result = parse_jest_output(json);
        assert_eq!(result.status, "pass");
        assert_eq!(result.ran, 5);
        assert_eq!(result.passed, 5);
        assert_eq!(result.failed, 0);
        assert!(result.failures.is_empty());
    }

    #[test]
    fn test_parse_with_failures() {
        let json = r#"{
            "numTotalTests": 3,
            "numPassedTests": 1,
            "numFailedTests": 2,
            "testResults": [{
                "name": "/src/__tests__/UserProfile.test.tsx",
                "assertionResults": [
                    {"ancestorTitles": ["UserProfile"], "title": "renders name", "status": "passed", "failureMessages": []},
                    {"ancestorTitles": ["UserProfile"], "title": "handles click", "status": "failed", "failureMessages": ["Expected 1 to be 2"]}
                ]
            }]
        }"#;
        let result = parse_jest_output(json);
        assert_eq!(result.status, "fail");
        assert_eq!(result.failed, 2);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].test_name, "UserProfile > handles click");
        assert_eq!(result.failures[0].message, "Expected 1 to be 2");
    }

    #[test]
    fn test_parse_prefixed_output() {
        let stdout = "console.log some noise\n{\"numTotalTests\":1,\"numPassedTests\":1,\"numFailedTests\":0,\"testResults\":[]}";
        let result = parse_jest_output(stdout);
        assert_eq!(result.status, "pass");
        assert_eq!(result.ran, 1);
    }
}
