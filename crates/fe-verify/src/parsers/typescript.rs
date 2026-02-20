use crate::types::{DiagnosticItem, StepResult};
use regex::Regex;
use std::sync::LazyLock;

/// Regex for tsc output with `--pretty false`:
///   src/file.ts(10,5): error TS2345: Some message.
static TSC_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.+?)\((\d+),(\d+)\):\s+(error|warning)\s+(TS\d+):\s+(.+)$").unwrap()
});

pub fn parse_tsc_output(stdout: &str) -> StepResult {
    let mut errors = Vec::new();
    let mut error_count = 0usize;
    let mut warning_count = 0usize;

    for line in stdout.lines() {
        let line = line.trim();
        if let Some(caps) = TSC_LINE.captures(line) {
            let severity_str = caps.get(4).unwrap().as_str();
            if severity_str == "error" {
                error_count += 1;
            } else {
                warning_count += 1;
            }

            errors.push(DiagnosticItem {
                file: caps.get(1).unwrap().as_str().to_string(),
                line: caps.get(2).unwrap().as_str().parse().unwrap_or(0),
                column: caps.get(3).unwrap().as_str().parse().unwrap_or(0),
                message: caps.get(6).unwrap().as_str().to_string(),
                rule: Some(caps.get(5).unwrap().as_str().to_string()),
                severity: severity_str.to_string(),
                suggestion: Some("Call fe_doctor with this error for a structured fix".into()),
            });
        }
    }

    let status = if error_count > 0 { "fail" } else { "pass" };

    StepResult {
        status: status.into(),
        error_count,
        warning_count,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clean() {
        let result = parse_tsc_output("");
        assert_eq!(result.status, "pass");
        assert_eq!(result.error_count, 0);
    }

    #[test]
    fn test_parse_type_errors() {
        let output = r#"src/components/UserProfile.tsx(10,5): error TS2345: Argument of type 'string' is not assignable to parameter of type 'number'.
src/hooks/useAuth.ts(22,3): error TS2322: Type 'undefined' is not assignable to type 'User'."#;
        let result = parse_tsc_output(output);
        assert_eq!(result.status, "fail");
        assert_eq!(result.error_count, 2);
        assert_eq!(result.errors.len(), 2);
        assert_eq!(result.errors[0].file, "src/components/UserProfile.tsx");
        assert_eq!(result.errors[0].line, 10);
        assert_eq!(result.errors[0].column, 5);
        assert_eq!(result.errors[0].rule.as_deref(), Some("TS2345"));
    }

    #[test]
    fn test_ignores_non_error_lines() {
        let output = "Found 2 errors in 1 file.\n\nErrors  Files\n     2  src/App.tsx";
        let result = parse_tsc_output(output);
        assert_eq!(result.status, "pass");
        assert_eq!(result.errors.len(), 0);
    }
}
