//! `wrap_in_block` operation.
//!
//! Wraps a range of statements (by line numbers) in a control structure
//! (if, try-catch, for, plain block).

use crate::edit::TextEdit;
use crate::format;
use crate::operations::{Executable, OperationError};
use tree_sitter::Tree;

/// The kind of wrapping block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrapKind {
    /// `if (condition) { ... }`
    If { condition: String },
    /// `try { ... } catch (e) { ... }`
    TryCatch { catch_param: String },
    /// `for (const item of iterable) { ... }`
    ForOf { item: String, iterable: String },
    /// `{ ... }` -- plain block
    Block,
}

/// The wrap_in_block operation.
pub struct WrapInBlock {
    pub start_line: usize, // 1-indexed
    pub end_line: usize,   // 1-indexed, inclusive
    pub wrap_kind: WrapKind,
}

impl WrapInBlock {
    pub fn new(start_line: usize, end_line: usize, wrap_kind: WrapKind) -> Self {
        Self {
            start_line,
            end_line,
            wrap_kind,
        }
    }
}

impl Executable for WrapInBlock {
    fn compute_edits(
        &self,
        source: &str,
        _tree: &Tree,
    ) -> Result<Vec<TextEdit>, OperationError> {
        if self.start_line == 0 || self.end_line == 0 || self.start_line > self.end_line {
            return Err(OperationError::InvalidParams {
                message: format!(
                    "Invalid line range: {}-{} (1-indexed, start <= end)",
                    self.start_line, self.end_line
                ),
            });
        }

        let lines: Vec<&str> = source.lines().collect();

        if self.end_line > lines.len() {
            return Err(OperationError::InvalidParams {
                message: format!(
                    "Line {} is out of range (file has {} lines)",
                    self.end_line,
                    lines.len()
                ),
            });
        }

        // Find byte offsets for the line range
        let start_byte = line_start_byte(source, self.start_line);
        let end_byte = line_end_byte(source, self.end_line);

        // Detect indentation from the first line
        let first_line = lines[self.start_line - 1];
        let base_indent = extract_leading_whitespace(first_line);
        let indent_style = format::infer_indent_style(source);
        let indent_unit = match indent_style {
            format::IndentStyle::Spaces(n) => " ".repeat(n as usize),
            format::IndentStyle::Tabs => "\t".to_string(),
        };

        // Extract the wrapped lines, re-indented one level deeper
        let wrapped_lines: Vec<String> = (self.start_line..=self.end_line)
            .map(|i| {
                let line = lines[i - 1];
                let trimmed = line.strip_prefix(base_indent).unwrap_or(line);
                format!("{}{}{}", base_indent, indent_unit, trimmed)
            })
            .collect();
        let wrapped_body = wrapped_lines.join("\n");

        // Build the replacement
        let replacement = match &self.wrap_kind {
            WrapKind::If { condition } => {
                format!(
                    "{}if ({}) {{\n{}\n{}}}",
                    base_indent, condition, wrapped_body, base_indent
                )
            }
            WrapKind::TryCatch { catch_param } => {
                format!(
                    "{}try {{\n{}\n{}}} catch ({}) {{\n{}{}\n{}}}",
                    base_indent,
                    wrapped_body,
                    base_indent,
                    catch_param,
                    base_indent,
                    indent_unit,
                    base_indent
                )
            }
            WrapKind::ForOf { item, iterable } => {
                format!(
                    "{}for (const {} of {}) {{\n{}\n{}}}",
                    base_indent, item, iterable, wrapped_body, base_indent
                )
            }
            WrapKind::Block => {
                format!(
                    "{}{{\n{}\n{}}}",
                    base_indent, wrapped_body, base_indent
                )
            }
        };

        Ok(vec![TextEdit {
            start: start_byte,
            end: end_byte,
            replacement,
            label: format!(
                "wrap lines {}-{} in {:?}",
                self.start_line, self.end_line, self.wrap_kind_name()
            ),
            priority: 0,
        }])
    }
}

impl WrapInBlock {
    fn wrap_kind_name(&self) -> &str {
        match &self.wrap_kind {
            WrapKind::If { .. } => "if",
            WrapKind::TryCatch { .. } => "try-catch",
            WrapKind::ForOf { .. } => "for-of",
            WrapKind::Block => "block",
        }
    }
}

/// Get the byte offset of the start of a 1-indexed line.
fn line_start_byte(source: &str, line: usize) -> usize {
    let mut current_line = 1;
    for (i, c) in source.char_indices() {
        if current_line == line {
            return i;
        }
        if c == '\n' {
            current_line += 1;
        }
    }
    source.len()
}

/// Get the byte offset past the end of a 1-indexed line (including newline if present).
fn line_end_byte(source: &str, line: usize) -> usize {
    let mut current_line = 1;
    for (i, c) in source.char_indices() {
        if c == '\n' {
            if current_line == line {
                return i; // Don't include the newline in the replacement
            }
            current_line += 1;
        }
    }
    // Last line (no trailing newline)
    if current_line == line {
        return source.len();
    }
    source.len()
}

/// Extract leading whitespace from a line.
fn extract_leading_whitespace(line: &str) -> &str {
    let trimmed = line.trim_start();
    &line[..line.len() - trimmed.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::EditSet;
    use tree_sitter::Parser;

    fn parse_ts(source: &str) -> Tree {
        let mut parser = Parser::new();
        let lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        parser.set_language(&lang).unwrap();
        parser.parse(source, None).unwrap()
    }

    fn apply(source: &str, edits: Vec<TextEdit>) -> String {
        if edits.is_empty() {
            return source.to_string();
        }
        let edit_set = EditSet::new(edits, source.len()).unwrap();
        edit_set.apply(source)
    }

    #[test]
    fn test_wrap_in_if() {
        let source = "function foo() {\n  doA();\n  doB();\n}\n";
        let tree = parse_ts(source);
        let op = WrapInBlock::new(
            2,
            3,
            WrapKind::If {
                condition: "isReady".to_string(),
            },
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("if (isReady) {"));
        assert!(result.contains("    doA();"));
        assert!(result.contains("    doB();"));
    }

    #[test]
    fn test_wrap_in_try_catch() {
        let source = "function foo() {\n  riskyCall();\n}\n";
        let tree = parse_ts(source);
        let op = WrapInBlock::new(
            2,
            2,
            WrapKind::TryCatch {
                catch_param: "error".to_string(),
            },
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("try {"));
        assert!(result.contains("} catch (error) {"));
        assert!(result.contains("    riskyCall();"));
    }

    #[test]
    fn test_wrap_in_for_of() {
        let source = "function process() {\n  console.log(item);\n}\n";
        let tree = parse_ts(source);
        let op = WrapInBlock::new(
            2,
            2,
            WrapKind::ForOf {
                item: "item".to_string(),
                iterable: "items".to_string(),
            },
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("for (const item of items) {"));
    }

    #[test]
    fn test_wrap_in_plain_block() {
        let source = "function foo() {\n  const a = 1;\n  const b = 2;\n}\n";
        let tree = parse_ts(source);
        let op = WrapInBlock::new(2, 3, WrapKind::Block);
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("  {\n    const a = 1;\n    const b = 2;\n  }"));
    }

    #[test]
    fn test_wrap_invalid_range() {
        let source = "function foo() {}\n";
        let tree = parse_ts(source);
        let op = WrapInBlock::new(3, 1, WrapKind::Block);
        let result = op.compute_edits(source, &tree);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrap_result_parses_cleanly() {
        let source = "function foo() {\n  const x = fetchData();\n  processData(x);\n}\n";
        let tree = parse_ts(source);
        let op = WrapInBlock::new(
            2,
            3,
            WrapKind::TryCatch {
                catch_param: "e".to_string(),
            },
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);

        let tree2 = parse_ts(&result);
        assert!(
            !tree2.root_node().has_error(),
            "Result has syntax errors:\n{}",
            result
        );
    }
}
