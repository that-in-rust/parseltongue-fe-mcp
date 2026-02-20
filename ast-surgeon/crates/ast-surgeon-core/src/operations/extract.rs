//! `extract_to_variable` operation.
//!
//! Extracts an expression (found by line and column, or by text pattern)
//! into a named `const` or `let` variable declaration.

use crate::edit::TextEdit;
use crate::operations::{Executable, OperationError};
use tree_sitter::{Node, Tree};

/// Variable declaration kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarKind {
    Const,
    Let,
}

impl VarKind {
    pub fn keyword(&self) -> &str {
        match self {
            Self::Const => "const",
            Self::Let => "let",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, OperationError> {
        match s.to_lowercase().as_str() {
            "const" => Ok(Self::Const),
            "let" => Ok(Self::Let),
            other => Err(OperationError::InvalidParams {
                message: format!("Invalid var_kind '{}', expected 'const' or 'let'", other),
            }),
        }
    }
}

/// The extract_to_variable operation.
///
/// Finds an expression at the given location and extracts it into a
/// variable declaration inserted before the containing statement.
pub struct ExtractToVariable {
    /// The exact expression text to extract.
    pub expression: String,
    /// Name for the new variable.
    pub variable_name: String,
    /// const or let
    pub var_kind: VarKind,
    /// Optional type annotation
    pub type_annotation: Option<String>,
}

impl ExtractToVariable {
    pub fn new(
        expression: String,
        variable_name: String,
        var_kind: VarKind,
        type_annotation: Option<String>,
    ) -> Self {
        Self {
            expression,
            variable_name,
            var_kind,
            type_annotation,
        }
    }
}

impl Executable for ExtractToVariable {
    fn compute_edits(
        &self,
        source: &str,
        tree: &Tree,
    ) -> Result<Vec<TextEdit>, OperationError> {
        // Find the expression in the source text
        let expr_byte_start = source
            .find(&self.expression)
            .ok_or_else(|| OperationError::TargetNotFound {
                description: format!(
                    "Expression '{}' not found in source",
                    self.expression
                ),
            })?;
        let expr_byte_end = expr_byte_start + self.expression.len();

        // Find the containing statement to determine where to insert the declaration
        let root = tree.root_node();
        let expr_node = root
            .descendant_for_byte_range(expr_byte_start, expr_byte_end)
            .ok_or_else(|| OperationError::TargetNotFound {
                description: "Could not find AST node for expression".to_string(),
            })?;

        // Walk up to find the containing statement
        let statement = find_containing_statement(&expr_node)
            .ok_or_else(|| OperationError::TargetNotFound {
                description: "Could not find containing statement for expression".to_string(),
            })?;

        // Determine indentation of the statement
        let stmt_start = statement.start_byte();
        let line_start = source[..stmt_start]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let indent = &source[line_start..stmt_start];

        // Build the variable declaration
        let type_suffix = match &self.type_annotation {
            Some(ty) => format!(": {}", ty),
            None => String::new(),
        };
        let declaration = format!(
            "{}{} {}{} = {};\n",
            indent,
            self.var_kind.keyword(),
            self.variable_name,
            type_suffix,
            self.expression
        );

        let mut edits = Vec::new();

        // 1. Insert variable declaration before the statement
        edits.push(TextEdit {
            start: line_start,
            end: line_start,
            replacement: declaration,
            label: format!("declare {} '{}'", self.var_kind.keyword(), self.variable_name),
            priority: 0,
        });

        // 2. Replace the expression with the variable name
        edits.push(TextEdit {
            start: expr_byte_start,
            end: expr_byte_end,
            replacement: self.variable_name.clone(),
            label: format!("replace expression with '{}'", self.variable_name),
            priority: 0,
        });

        Ok(edits)
    }
}

/// Walk up the tree to find the nearest statement-level ancestor.
fn find_containing_statement<'a>(node: &'a Node<'a>) -> Option<Node<'a>> {
    let mut current = *node;
    loop {
        let kind = current.kind();
        if is_statement_kind(kind) {
            return Some(current);
        }
        current = current.parent()?;
    }
}

fn is_statement_kind(kind: &str) -> bool {
    matches!(
        kind,
        "expression_statement"
            | "variable_declaration"
            | "lexical_declaration"
            | "return_statement"
            | "if_statement"
            | "for_statement"
            | "for_in_statement"
            | "while_statement"
            | "do_statement"
            | "switch_statement"
            | "throw_statement"
            | "try_statement"
            | "export_statement"
    )
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
    fn test_extract_simple_expression() {
        let source = "function foo() {\n  console.log(1 + 2);\n}\n";
        let tree = parse_ts(source);
        let op = ExtractToVariable::new(
            "1 + 2".to_string(),
            "sum".to_string(),
            VarKind::Const,
            None,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("const sum = 1 + 2;"));
        assert!(result.contains("console.log(sum)"));
    }

    #[test]
    fn test_extract_with_type_annotation() {
        let source = "function foo() {\n  return getData();\n}\n";
        let tree = parse_ts(source);
        let op = ExtractToVariable::new(
            "getData()".to_string(),
            "data".to_string(),
            VarKind::Const,
            Some("Data".to_string()),
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("const data: Data = getData();"));
        assert!(result.contains("return data;"));
    }

    #[test]
    fn test_extract_with_let() {
        let source = "function foo() {\n  process(getItems());\n}\n";
        let tree = parse_ts(source);
        let op = ExtractToVariable::new(
            "getItems()".to_string(),
            "items".to_string(),
            VarKind::Let,
            None,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("let items = getItems();"));
        assert!(result.contains("process(items)"));
    }

    #[test]
    fn test_extract_not_found() {
        let source = "function foo() {\n  console.log('hello');\n}\n";
        let tree = parse_ts(source);
        let op = ExtractToVariable::new(
            "nonexistent".to_string(),
            "x".to_string(),
            VarKind::Const,
            None,
        );
        let result = op.compute_edits(source, &tree);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_result_parses_cleanly() {
        let source =
            "function calculate() {\n  return Math.sqrt(a * a + b * b);\n}\n";
        let tree = parse_ts(source);
        let op = ExtractToVariable::new(
            "a * a + b * b".to_string(),
            "sumOfSquares".to_string(),
            VarKind::Const,
            Some("number".to_string()),
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
