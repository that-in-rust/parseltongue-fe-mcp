//! Post-edit validation: re-parse and check for syntax errors.

use serde::Serialize;
use thiserror::Error;
use tree_sitter::{Parser, Tree};

/// Errors found during validation.
#[derive(Debug, Clone, Error, Serialize)]
pub enum ValidationError {
    #[error("tree-sitter parser returned None (timeout or cancellation)")]
    ParseFailed,
    #[error("Result contains {count} syntax error(s)")]
    SyntaxErrors {
        count: usize,
        errors: Vec<SyntaxError>,
    },
}

/// A single syntax error location in the parsed output.
#[derive(Debug, Clone, Serialize)]
pub struct SyntaxError {
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column number.
    pub column: usize,
    /// ~60 chars of surrounding source for context.
    pub context: String,
    /// The tree-sitter node kind (e.g., "ERROR", "MISSING").
    pub node_kind: String,
}

/// Verify that `source` parses cleanly with the given language.
///
/// Returns `Ok(tree)` if the parse tree has no ERROR or MISSING nodes,
/// or `Err` with details about what went wrong.
pub fn verify_parse(
    source: &str,
    language: &tree_sitter::Language,
) -> Result<Tree, ValidationError> {
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .expect("language version mismatch");

    let tree = parser.parse(source, None).ok_or(ValidationError::ParseFailed)?;

    let root = tree.root_node();
    if root.has_error() {
        let errors = collect_error_nodes(&root, source);
        return Err(ValidationError::SyntaxErrors {
            count: errors.len(),
            errors,
        });
    }

    Ok(tree)
}

/// Parse source without validation (best-effort, may contain errors).
pub fn parse_best_effort(
    source: &str,
    language: &tree_sitter::Language,
) -> Result<Tree, ValidationError> {
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .expect("language version mismatch");

    parser.parse(source, None).ok_or(ValidationError::ParseFailed)
}

/// Count ERROR nodes in a tree (useful for checking if source already has errors).
pub fn count_errors(tree: &Tree) -> usize {
    let root = tree.root_node();
    if !root.has_error() {
        return 0;
    }
    collect_error_nodes(&root, "").len()
}

fn collect_error_nodes(node: &tree_sitter::Node, source: &str) -> Vec<SyntaxError> {
    let mut errors = Vec::new();
    collect_errors_recursive(node, source, &mut errors);
    errors
}

fn collect_errors_recursive(
    node: &tree_sitter::Node,
    source: &str,
    errors: &mut Vec<SyntaxError>,
) {
    if node.is_error() || node.is_missing() {
        let start = node.start_position();
        let context = if !source.is_empty() {
            let byte_start = node.start_byte().saturating_sub(30);
            let byte_end = (node.end_byte() + 30).min(source.len());
            // Clamp to valid UTF-8 boundaries
            let byte_start = floor_char_boundary(source, byte_start);
            let byte_end = ceil_char_boundary(source, byte_end);
            source[byte_start..byte_end].to_string()
        } else {
            String::new()
        };
        errors.push(SyntaxError {
            line: start.row + 1,
            column: start.column + 1,
            context,
            node_kind: node.kind().to_string(),
        });
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_errors_recursive(&cursor.node(), source, errors);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Find the largest byte index <= `idx` that is a char boundary.
fn floor_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Find the smallest byte index >= `idx` that is a char boundary.
fn ceil_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}
