//! `add_parameter` and `remove_parameter` operations.
//!
//! Modifies function signatures: adds or removes parameters.
//! Handles regular functions, arrow functions, and class methods.

use crate::edit::TextEdit;
use crate::operations::{Executable, OperationError};
use tree_sitter::{Node, Tree};

/// Position for a new parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamPosition {
    /// Insert at the beginning (before all existing params).
    First,
    /// Insert at the end (after all existing params, before rest param if any).
    Last,
    /// Insert at a specific 0-based index.
    Index(usize),
}

impl ParamPosition {
    pub fn from_str(s: &str) -> Result<Self, OperationError> {
        match s.to_lowercase().as_str() {
            "first" => Ok(Self::First),
            "last" => Ok(Self::Last),
            _ => {
                if let Ok(i) = s.parse::<usize>() {
                    Ok(Self::Index(i))
                } else {
                    Err(OperationError::InvalidParams {
                        message: format!(
                            "Invalid position '{}', expected 'first', 'last', or a number",
                            s
                        ),
                    })
                }
            }
        }
    }
}

/// The add_parameter operation.
pub struct AddParameter {
    pub function_name: String,
    pub param_name: String,
    pub param_type: Option<String>,
    pub default_value: Option<String>,
    pub position: ParamPosition,
}

impl AddParameter {
    pub fn new(
        function_name: String,
        param_name: String,
        param_type: Option<String>,
        default_value: Option<String>,
        position: ParamPosition,
    ) -> Self {
        Self {
            function_name,
            param_name,
            param_type,
            default_value,
            position,
        }
    }

    /// Format the parameter text, e.g. "name: string" or "name: string = 'default'"
    fn format_param(&self) -> String {
        let mut param = self.param_name.clone();
        if let Some(ref ty) = self.param_type {
            param = format!("{}: {}", param, ty);
        }
        if let Some(ref default) = self.default_value {
            param = format!("{} = {}", param, default);
        }
        param
    }
}

impl Executable for AddParameter {
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

        let params_node = find_formal_parameters(&func_node)
            .ok_or_else(|| OperationError::TargetNotFound {
                description: format!(
                    "Could not find parameter list for '{}'",
                    self.function_name
                ),
            })?;

        let param_text = self.format_param();

        // Collect existing parameter nodes (skip ( and ) and ,)
        let existing_params = collect_param_nodes(&params_node);

        // Check for duplicate
        for p in &existing_params {
            let p_text = &source[p.start_byte()..p.end_byte()];
            let p_name = p_text.split(':').next().unwrap_or(p_text).trim();
            if p_name == self.param_name {
                return Ok(vec![]); // Already exists -- no-op
            }
        }

        if existing_params.is_empty() {
            // Empty params: insert between ( and )
            let insert_pos = params_node.start_byte() + 1; // after '('
            return Ok(vec![TextEdit {
                start: insert_pos,
                end: insert_pos,
                replacement: param_text,
                label: format!("add parameter '{}' to '{}'", self.param_name, self.function_name),
                priority: 0,
            }]);
        }

        // Determine insertion index
        let insert_idx = match self.position {
            ParamPosition::First => 0,
            ParamPosition::Last => existing_params.len(),
            ParamPosition::Index(i) => i.min(existing_params.len()),
        };

        if insert_idx == 0 {
            // Insert before first param
            let first = &existing_params[0];
            Ok(vec![TextEdit {
                start: first.start_byte(),
                end: first.start_byte(),
                replacement: format!("{}, ", param_text),
                label: format!("add parameter '{}' to '{}'", self.param_name, self.function_name),
                priority: 0,
            }])
        } else if insert_idx >= existing_params.len() {
            // Insert after last param
            let last = &existing_params[existing_params.len() - 1];
            Ok(vec![TextEdit {
                start: last.end_byte(),
                end: last.end_byte(),
                replacement: format!(", {}", param_text),
                label: format!("add parameter '{}' to '{}'", self.param_name, self.function_name),
                priority: 0,
            }])
        } else {
            // Insert before the param at insert_idx
            let target = &existing_params[insert_idx];
            Ok(vec![TextEdit {
                start: target.start_byte(),
                end: target.start_byte(),
                replacement: format!("{}, ", param_text),
                label: format!("add parameter '{}' to '{}'", self.param_name, self.function_name),
                priority: 0,
            }])
        }
    }
}

/// The remove_parameter operation.
pub struct RemoveParameter {
    pub function_name: String,
    pub param_name: String,
}

impl RemoveParameter {
    pub fn new(function_name: String, param_name: String) -> Self {
        Self {
            function_name,
            param_name,
        }
    }
}

impl Executable for RemoveParameter {
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

        let params_node = find_formal_parameters(&func_node)
            .ok_or_else(|| OperationError::TargetNotFound {
                description: format!(
                    "Could not find parameter list for '{}'",
                    self.function_name
                ),
            })?;

        let existing_params = collect_param_nodes(&params_node);

        // Find the parameter to remove
        let mut target_idx = None;
        for (i, p) in existing_params.iter().enumerate() {
            let p_text = &source[p.start_byte()..p.end_byte()];
            let p_name = p_text.split(':').next().unwrap_or(p_text).trim();
            if p_name == self.param_name {
                target_idx = Some(i);
                break;
            }
        }

        let idx = target_idx.ok_or_else(|| OperationError::TargetNotFound {
            description: format!(
                "Parameter '{}' not found in function '{}'",
                self.param_name, self.function_name
            ),
        })?;

        let total = existing_params.len();

        if total == 1 {
            // Only param -- remove it, leave empty parens
            let param = &existing_params[0];
            return Ok(vec![TextEdit {
                start: param.start_byte(),
                end: param.end_byte(),
                replacement: String::new(),
                label: format!(
                    "remove parameter '{}' from '{}'",
                    self.param_name, self.function_name
                ),
                priority: 0,
            }]);
        }

        if idx == total - 1 {
            // Last param -- remove preceding comma + space + the param
            let prev = &existing_params[idx - 1];
            let param = &existing_params[idx];
            Ok(vec![TextEdit {
                start: prev.end_byte(),
                end: param.end_byte(),
                replacement: String::new(),
                label: format!(
                    "remove parameter '{}' from '{}'",
                    self.param_name, self.function_name
                ),
                priority: 0,
            }])
        } else {
            // Not the last -- remove the param + following comma + space
            let param = &existing_params[idx];
            let next = &existing_params[idx + 1];
            Ok(vec![TextEdit {
                start: param.start_byte(),
                end: next.start_byte(),
                replacement: String::new(),
                label: format!(
                    "remove parameter '{}' from '{}'",
                    self.param_name, self.function_name
                ),
                priority: 0,
            }])
        }
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
        // function declaration: function foo() {}
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let n = &source[name_node.start_byte()..name_node.end_byte()];
                if n == name {
                    return Some(node);
                }
            }
        }
        // method_definition: class { foo() {} }
        "method_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let n = &source[name_node.start_byte()..name_node.end_byte()];
                if n == name {
                    return Some(node);
                }
            }
        }
        // variable declaration with arrow function or function expression:
        // const foo = () => {} or const foo = function() {}
        "variable_declarator" | "lexical_declaration" => {
            // For lexical_declaration, check its children (variable_declarator)
        }
        // export function foo() {}
        "export_statement" => {
            // Will recurse into children
        }
        _ => {}
    }

    // Check for `const foo = ...` pattern
    if node.kind() == "variable_declarator" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let n = &source[name_node.start_byte()..name_node.end_byte()];
            if n == name {
                if let Some(value) = node.child_by_field_name("value") {
                    if value.kind() == "arrow_function"
                        || value.kind() == "function_expression"
                        || value.kind() == "generator_function"
                    {
                        return Some(value);
                    }
                }
            }
        }
    }

    // Recurse
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

/// Find the formal_parameters node inside a function/arrow node.
fn find_formal_parameters<'a>(func_node: &'a Node<'a>) -> Option<Node<'a>> {
    // function_declaration/function_expression: has "parameters" field
    if let Some(params) = func_node.child_by_field_name("parameters") {
        return Some(params);
    }

    // arrow_function: first child of kind formal_parameters (if parens present)
    // or just a single identifier (no parens)
    let mut cursor = func_node.walk();
    if cursor.goto_first_child() {
        loop {
            if cursor.node().kind() == "formal_parameters" {
                return Some(cursor.node());
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    None
}

/// Collect actual parameter nodes from a formal_parameters node
/// (excluding punctuation like `(`, `)`, `,`).
fn collect_param_nodes<'a>(params_node: &'a Node<'a>) -> Vec<Node<'a>> {
    let mut params = Vec::new();
    let mut cursor = params_node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            match child.kind() {
                "required_parameter"
                | "optional_parameter"
                | "rest_pattern"
                | "identifier"
                | "assignment_pattern"
                | "object_pattern"
                | "array_pattern" => {
                    params.push(child);
                }
                _ => {}
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    params
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

    // --- add_parameter tests ---

    #[test]
    fn test_add_param_to_empty_function() {
        let source = "function greet() {\n  console.log('hi');\n}\n";
        let tree = parse_ts(source);
        let op = AddParameter::new(
            "greet".to_string(),
            "name".to_string(),
            Some("string".to_string()),
            None,
            ParamPosition::Last,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("function greet(name: string)"));
    }

    #[test]
    fn test_add_param_last() {
        let source = "function add(a: number, b: number) {\n  return a + b;\n}\n";
        let tree = parse_ts(source);
        let op = AddParameter::new(
            "add".to_string(),
            "c".to_string(),
            Some("number".to_string()),
            Some("0".to_string()),
            ParamPosition::Last,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("a: number, b: number, c: number = 0"));
    }

    #[test]
    fn test_add_param_first() {
        let source = "function greet(name: string) {}\n";
        let tree = parse_ts(source);
        let op = AddParameter::new(
            "greet".to_string(),
            "prefix".to_string(),
            Some("string".to_string()),
            None,
            ParamPosition::First,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("(prefix: string, name: string)"));
    }

    #[test]
    fn test_add_param_duplicate_is_noop() {
        let source = "function greet(name: string) {}\n";
        let tree = parse_ts(source);
        let op = AddParameter::new(
            "greet".to_string(),
            "name".to_string(),
            Some("string".to_string()),
            None,
            ParamPosition::Last,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        assert!(edits.is_empty());
    }

    #[test]
    fn test_add_param_to_arrow_function() {
        let source = "const greet = (name: string) => {\n  console.log(name);\n};\n";
        let tree = parse_ts(source);
        let op = AddParameter::new(
            "greet".to_string(),
            "loud".to_string(),
            Some("boolean".to_string()),
            Some("false".to_string()),
            ParamPosition::Last,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("name: string, loud: boolean = false"));
    }

    #[test]
    fn test_add_param_not_found() {
        let source = "function greet() {}\n";
        let tree = parse_ts(source);
        let op = AddParameter::new(
            "nonexistent".to_string(),
            "x".to_string(),
            None,
            None,
            ParamPosition::Last,
        );
        let result = op.compute_edits(source, &tree);
        assert!(result.is_err());
    }

    // --- remove_parameter tests ---

    #[test]
    fn test_remove_only_param() {
        let source = "function greet(name: string) {}\n";
        let tree = parse_ts(source);
        let op = RemoveParameter::new("greet".to_string(), "name".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("function greet()"));
    }

    #[test]
    fn test_remove_first_param() {
        let source = "function add(a: number, b: number) {}\n";
        let tree = parse_ts(source);
        let op = RemoveParameter::new("add".to_string(), "a".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("(b: number)"));
        assert!(!result.contains("a:"));
    }

    #[test]
    fn test_remove_last_param() {
        let source = "function add(a: number, b: number) {}\n";
        let tree = parse_ts(source);
        let op = RemoveParameter::new("add".to_string(), "b".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("(a: number)"));
        assert!(!result.contains("b:"));
    }

    #[test]
    fn test_remove_middle_param() {
        let source = "function calc(a: number, b: number, c: number) {}\n";
        let tree = parse_ts(source);
        let op = RemoveParameter::new("calc".to_string(), "b".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("(a: number, c: number)"));
    }

    #[test]
    fn test_remove_param_not_found() {
        let source = "function greet(name: string) {}\n";
        let tree = parse_ts(source);
        let op = RemoveParameter::new("greet".to_string(), "age".to_string());
        let result = op.compute_edits(source, &tree);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_param_from_arrow() {
        let source = "const greet = (name: string, loud: boolean) => {};\n";
        let tree = parse_ts(source);
        let op = RemoveParameter::new("greet".to_string(), "loud".to_string());
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("(name: string)"));
        assert!(!result.contains("loud"));
    }

    // --- re-parse validation ---

    #[test]
    fn test_add_param_result_parses_cleanly() {
        let source = "export function fetchData(url: string) {\n  return fetch(url);\n}\n";
        let tree = parse_ts(source);
        let op = AddParameter::new(
            "fetchData".to_string(),
            "options".to_string(),
            Some("RequestInit".to_string()),
            Some("{}".to_string()),
            ParamPosition::Last,
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

    #[test]
    fn test_remove_param_result_parses_cleanly() {
        let source = "export function fetchData(url: string, options: RequestInit) {\n  return fetch(url, options);\n}\n";
        let tree = parse_ts(source);
        let op = RemoveParameter::new("fetchData".to_string(), "options".to_string());
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
