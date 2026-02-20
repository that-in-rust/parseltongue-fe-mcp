//! `make_async` operation.
//!
//! Adds the `async` keyword to a function and optionally wraps
//! its return type annotation in `Promise<>`.

use crate::edit::TextEdit;
use crate::operations::{Executable, OperationError};
use tree_sitter::{Node, Tree};

/// The make_async operation.
pub struct MakeAsync {
    pub function_name: String,
}

impl MakeAsync {
    pub fn new(function_name: String) -> Self {
        Self { function_name }
    }
}

impl Executable for MakeAsync {
    fn compute_edits(
        &self,
        source: &str,
        tree: &Tree,
    ) -> Result<Vec<TextEdit>, OperationError> {
        let root = tree.root_node();
        let func_node = find_function_by_name(&root, source, &self.function_name)
            .ok_or_else(|| OperationError::TargetNotFound {
                description: format!("Function '{}' not found", self.function_name),
            })?;

        // Check if already async

        // For function declarations, check if "async" keyword is already present
        if is_already_async(&func_node, source) {
            return Ok(vec![]); // Already async -- no-op
        }

        let mut edits = Vec::new();

        // Add `async` keyword
        match func_node.kind() {
            "function_declaration" | "generator_function_declaration" => {
                // Insert "async " before "function"
                edits.push(TextEdit {
                    start: func_node.start_byte(),
                    end: func_node.start_byte(),
                    replacement: "async ".to_string(),
                    label: format!("make '{}' async", self.function_name),
                    priority: 0,
                });
            }
            "arrow_function" => {
                // Insert "async " before the arrow function start
                edits.push(TextEdit {
                    start: func_node.start_byte(),
                    end: func_node.start_byte(),
                    replacement: "async ".to_string(),
                    label: format!("make '{}' async", self.function_name),
                    priority: 0,
                });
            }
            "function_expression" => {
                // Insert "async " before "function"
                edits.push(TextEdit {
                    start: func_node.start_byte(),
                    end: func_node.start_byte(),
                    replacement: "async ".to_string(),
                    label: format!("make '{}' async", self.function_name),
                    priority: 0,
                });
            }
            "method_definition" => {
                // For methods, the "async" keyword goes before the method name
                // Find the method name node
                if let Some(name_node) = func_node.child_by_field_name("name") {
                    edits.push(TextEdit {
                        start: name_node.start_byte(),
                        end: name_node.start_byte(),
                        replacement: "async ".to_string(),
                        label: format!("make '{}' async", self.function_name),
                        priority: 0,
                    });
                }
            }
            _ => {}
        }

        // Wrap return type in Promise<> if there is one
        if let Some(return_type) = find_return_type(&func_node) {
            let return_type_text =
                &source[return_type.start_byte()..return_type.end_byte()];
            // Don't wrap if already Promise<>
            if !return_type_text.starts_with("Promise<") {
                edits.push(TextEdit {
                    start: return_type.start_byte(),
                    end: return_type.end_byte(),
                    replacement: format!("Promise<{}>", return_type_text),
                    label: format!("wrap return type of '{}' in Promise<>", self.function_name),
                    priority: 0,
                });
            }
        }

        if edits.is_empty() {
            return Ok(vec![]); // Nothing to do
        }

        Ok(edits)
    }
}

// --- Helper functions ---

/// Find a function/arrow-function/method node by its name.
fn find_function_by_name<'a>(
    root: &'a Node<'a>,
    source: &str,
    name: &str,
) -> Option<Node<'a>> {
    let mut cursor = root.walk();
    find_function_recursive(&mut cursor, source, name)
}

fn find_function_recursive<'a>(
    cursor: &mut tree_sitter::TreeCursor<'a>,
    source: &str,
    name: &str,
) -> Option<Node<'a>> {
    let node = cursor.node();

    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let n = &source[name_node.start_byte()..name_node.end_byte()];
                if n == name {
                    return Some(node);
                }
            }
        }
        "method_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let n = &source[name_node.start_byte()..name_node.end_byte()];
                if n == name {
                    return Some(node);
                }
            }
        }
        "variable_declarator" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let n = &source[name_node.start_byte()..name_node.end_byte()];
                if n == name {
                    if let Some(value) = node.child_by_field_name("value") {
                        if value.kind() == "arrow_function"
                            || value.kind() == "function_expression"
                        {
                            return Some(value);
                        }
                    }
                }
            }
        }
        _ => {}
    }

    if cursor.goto_first_child() {
        loop {
            if let Some(found) = find_function_recursive(cursor, source, name) {
                return Some(found);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }

    None
}

/// Check if a function node is already async.
fn is_already_async(func_node: &Node, source: &str) -> bool {
    // Check if the function text starts with "async"
    let text = &source[func_node.start_byte()..func_node.end_byte()];
    if text.starts_with("async ") || text.starts_with("async\n") {
        return true;
    }

    // For method_definition, check children for "async" keyword
    let mut cursor = func_node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            let child_text = &source[child.start_byte()..child.end_byte()];
            if child_text == "async" {
                return true;
            }
            // Stop once we reach the function body or parameters
            if child.kind() == "formal_parameters" || child.kind() == "statement_block" {
                break;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    false
}

/// Find the return type annotation node of a function.
fn find_return_type<'a>(func_node: &'a Node<'a>) -> Option<Node<'a>> {
    // The return_type field contains the type annotation node
    if let Some(return_type) = func_node.child_by_field_name("return_type") {
        // return_type is a `type_annotation` node, containing `: Type`
        // We want just the type part (skip the `:`)
        let mut cursor = return_type.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                // Skip the colon
                if child.kind() != ":" {
                    return Some(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        return Some(return_type);
    }
    None
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
    fn test_make_function_async() {
        let source = "function fetchData(url: string) {\n  return fetch(url);\n}\n";
        let tree = parse_ts(source);
        let op = MakeAsync::new("fetchData".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("async function fetchData"));
    }

    #[test]
    fn test_make_arrow_async() {
        let source = "const fetchData = (url: string) => {\n  return fetch(url);\n};\n";
        let tree = parse_ts(source);
        let op = MakeAsync::new("fetchData".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("async (url: string) =>"));
    }

    #[test]
    fn test_already_async_is_noop() {
        let source = "async function fetchData(url: string) {\n  return fetch(url);\n}\n";
        let tree = parse_ts(source);
        let op = MakeAsync::new("fetchData".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        assert!(edits.is_empty());
    }

    #[test]
    fn test_make_async_wraps_return_type() {
        let source = "function fetchData(url: string): Response {\n  return fetch(url);\n}\n";
        let tree = parse_ts(source);
        let op = MakeAsync::new("fetchData".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("async function fetchData"));
        assert!(result.contains("Promise<Response>"));
    }

    #[test]
    fn test_make_async_already_promise_return_type() {
        let source =
            "function fetchData(url: string): Promise<Response> {\n  return fetch(url);\n}\n";
        let tree = parse_ts(source);
        let op = MakeAsync::new("fetchData".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("async function fetchData"));
        // Should NOT double-wrap
        assert!(result.contains("Promise<Response>"));
        assert!(!result.contains("Promise<Promise<"));
    }

    #[test]
    fn test_make_async_not_found() {
        let source = "function foo() {}\n";
        let tree = parse_ts(source);
        let op = MakeAsync::new("bar".to_string());
        let result = op.compute_edits(source, &tree);
        assert!(result.is_err());
    }

    #[test]
    fn test_make_async_exported_function() {
        let source = "export function fetchData(url: string) {\n  return fetch(url);\n}\n";
        let tree = parse_ts(source);
        let op = MakeAsync::new("fetchData".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("async function fetchData"));
    }

    // --- re-parse validation ---

    #[test]
    fn test_make_async_result_parses_cleanly() {
        let source = "export function fetchData(url: string): Response {\n  return fetch(url);\n}\n";
        let tree = parse_ts(source);
        let op = MakeAsync::new("fetchData".to_string());
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
