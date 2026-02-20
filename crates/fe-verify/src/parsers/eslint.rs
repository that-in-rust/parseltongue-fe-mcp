use crate::types::{DiagnosticItem, StepResult};
use serde::Deserialize;

/// Raw ESLint JSON output (from `eslint --format json`).
#[derive(Deserialize)]
struct ESLintFile {
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<ESLintMessage>,
    #[serde(rename = "errorCount")]
    error_count: usize,
    #[serde(rename = "warningCount")]
    warning_count: usize,
}

#[derive(Deserialize)]
struct ESLintMessage {
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    severity: u8, // 1 = warning, 2 = error
    message: String,
    line: usize,
    column: usize,
}

pub fn parse_eslint_output(stdout: &str) -> StepResult {
    let files: Vec<ESLintFile> = match serde_json::from_str(stdout) {
        Ok(f) => f,
        Err(e) => {
            return StepResult {
                status: "fail".into(),
                error_count: 1,
                warning_count: 0,
                errors: vec![DiagnosticItem {
                    file: String::new(),
                    line: 0,
                    column: 0,
                    message: format!("Failed to parse ESLint JSON output: {e}"),
                    rule: None,
                    severity: "error".into(),
                    suggestion: None,
                }],
            };
        }
    };

    let mut errors = Vec::new();
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;

    for file in &files {
        total_errors += file.error_count;
        total_warnings += file.warning_count;

        for msg in &file.messages {
            errors.push(DiagnosticItem {
                file: file.file_path.clone(),
                line: msg.line,
                column: msg.column,
                message: msg.message.clone(),
                rule: msg.rule_id.clone(),
                severity: if msg.severity >= 2 { "error" } else { "warning" }.into(),
                suggestion: Some("Call fe_doctor with this error for a structured fix".into()),
            });
        }
    }

    let status = if total_errors > 0 { "fail" } else { "pass" };

    StepResult {
        status: status.into(),
        error_count: total_errors,
        warning_count: total_warnings,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clean_output() {
        let json = r#"[{"filePath":"/src/App.tsx","messages":[],"errorCount":0,"warningCount":0}]"#;
        let result = parse_eslint_output(json);
        assert_eq!(result.status, "pass");
        assert_eq!(result.error_count, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_parse_errors() {
        let json = r#"[{
            "filePath": "/src/App.tsx",
            "messages": [
                {
                    "ruleId": "no-unused-vars",
                    "severity": 2,
                    "message": "'x' is defined but never used.",
                    "line": 3,
                    "column": 7
                },
                {
                    "ruleId": "react-hooks/exhaustive-deps",
                    "severity": 1,
                    "message": "React Hook useEffect has a missing dependency: 'userId'.",
                    "line": 12,
                    "column": 5
                }
            ],
            "errorCount": 1,
            "warningCount": 1
        }]"#;
        let result = parse_eslint_output(json);
        assert_eq!(result.status, "fail");
        assert_eq!(result.error_count, 1);
        assert_eq!(result.warning_count, 1);
        assert_eq!(result.errors.len(), 2);
        assert_eq!(result.errors[0].rule.as_deref(), Some("no-unused-vars"));
        assert_eq!(result.errors[0].severity, "error");
        assert_eq!(result.errors[1].severity, "warning");
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_eslint_output("not json at all");
        assert_eq!(result.status, "fail");
        assert_eq!(result.error_count, 1);
    }
}
