//! `add_import` and `remove_import` operations.

use crate::edit::TextEdit;
use crate::operations::{Executable, OperationError};
use tree_sitter::{Node, Tree};

/// The add_import operation.
///
/// Adds an import statement, or merges specifiers into an existing import
/// from the same source module.
pub struct AddImport {
    pub source_module: String,
    pub specifiers: Vec<String>,
    pub default_import: Option<String>,
    pub type_only: bool,
}

impl AddImport {
    pub fn new(
        source_module: String,
        specifiers: Vec<String>,
        default_import: Option<String>,
        type_only: bool,
    ) -> Self {
        Self {
            source_module,
            specifiers,
            default_import,
            type_only,
        }
    }
}

impl Executable for AddImport {
    fn compute_edits(
        &self,
        source: &str,
        tree: &Tree,
    ) -> Result<Vec<TextEdit>, OperationError> {
        if self.specifiers.is_empty() && self.default_import.is_none() {
            return Err(OperationError::InvalidParams {
                message: "add_import requires at least one specifier or a default import"
                    .to_string(),
            });
        }

        let root = tree.root_node();

        // Find existing import from the same source module
        if let Some(existing) = find_import_from_source(&root, source, &self.source_module) {
            self.merge_into_existing(source, &existing)
        } else {
            self.insert_new_import(source, tree)
        }
    }
}

impl AddImport {
    /// Merge specifiers into an existing import statement.
    fn merge_into_existing(
        &self,
        source: &str,
        import_node: &Node,
    ) -> Result<Vec<TextEdit>, OperationError> {
        // Find existing specifiers
        let existing_specifiers = extract_existing_specifiers(import_node, source);

        // Determine which specifiers are new
        let new_specifiers: Vec<&String> = self
            .specifiers
            .iter()
            .filter(|s| !existing_specifiers.contains(&s.as_str()))
            .collect();

        if new_specifiers.is_empty() && self.default_import.is_none() {
            return Ok(vec![]); // All specifiers already exist -- no-op
        }

        // Check if default import already exists
        let has_existing_default = has_default_import(import_node, source);
        let need_default = self.default_import.is_some() && !has_existing_default;

        if new_specifiers.is_empty() && !need_default {
            return Ok(vec![]); // Nothing to add
        }

        let mut edits = Vec::new();

        // If we need to add a default import, rewrite the entire import statement
        // (can't just insert into named_imports block for this case)
        if need_default {
            let all_specifiers: Vec<String> = existing_specifiers
                .iter()
                .map(|s| s.to_string())
                .chain(new_specifiers.iter().map(|s| s.to_string()))
                .collect();
            let quote = detect_quote_style(source);
            let semi = if detect_semicolons(source) { ";" } else { "" };
            let type_keyword = if self.type_only { "type " } else { "" };

            let mut parts = Vec::new();
            if let Some(ref default) = self.default_import {
                parts.push(default.clone());
            }
            if !all_specifiers.is_empty() {
                let specs = all_specifiers.join(", ");
                parts.push(format!("{{ {} }}", specs));
            }
            let import_clause = parts.join(", ");
            let new_import = format!(
                "import {}{} from {}{}{}{}",
                type_keyword, import_clause, quote, self.source_module, quote, semi
            );
            edits.push(TextEdit {
                start: import_node.start_byte(),
                end: import_node.end_byte(),
                replacement: new_import,
                label: format!("rewrite import from '{}' to add default", self.source_module),
                priority: 0,
            });
            return Ok(edits);
        }

        // Handle adding new named specifiers to existing named_imports block
        if !new_specifiers.is_empty() {
            let named_imports = find_child_by_kind(import_node, "import_clause")
                .and_then(|clause| {
                    // Walk children inline to avoid lifetime issues with find_child_by_kind
                    let mut c = clause.walk();
                    if !c.goto_first_child() {
                        return None;
                    }
                    loop {
                        if c.node().kind() == "named_imports" {
                            return Some(c.node());
                        }
                        if !c.goto_next_sibling() {
                            return None;
                        }
                    }
                });
            if let Some(named_imports) = named_imports {
                // Find the closing brace position
                let close_brace = named_imports.end_byte() - 1; // position of '}'
                let before_brace = &source[named_imports.start_byte()..close_brace];

                // Detect if there's a trailing comma
                let trimmed = before_brace.trim_end();
                let has_trailing_comma = trimmed.ends_with(',');

                // Build the insertion text
                let _separator = if has_trailing_comma { " " } else { ", " };
                let new_text = new_specifiers
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");

                let insertion = if has_trailing_comma {
                    format!("{}{}", new_text, if has_trailing_comma { ", " } else { "" })
                } else {
                    format!(", {}", new_text)
                };

                // Insert before the closing brace
                edits.push(TextEdit {
                    start: close_brace,
                    end: close_brace,
                    replacement: insertion,
                    label: format!("add specifiers to import from '{}'", self.source_module),
                    priority: 0,
                });
            } else {
                // No named_imports block exists (maybe only default import).
                // We need to add { specifiers } after the existing import clause.
                // Replace the entire import statement.
                let new_import = self.format_full_import(source, &existing_specifiers);
                edits.push(TextEdit {
                    start: import_node.start_byte(),
                    end: import_node.end_byte(),
                    replacement: new_import,
                    label: format!("rewrite import from '{}'", self.source_module),
                    priority: 0,
                });
            }
        }

        Ok(edits)
    }

    /// Insert a brand new import statement.
    fn insert_new_import(
        &self,
        source: &str,
        tree: &Tree,
    ) -> Result<Vec<TextEdit>, OperationError> {
        let insertion_point = find_import_insertion_point(source, tree);
        let import_text = self.format_full_import(source, &[]);

        // Add newline handling
        let needs_leading_newline =
            insertion_point > 0 && source.as_bytes().get(insertion_point - 1) != Some(&b'\n');
        let needs_trailing_newline = insertion_point < source.len()
            && source.as_bytes().get(insertion_point) != Some(&b'\n');

        let mut text = String::new();
        if needs_leading_newline {
            text.push('\n');
        }
        text.push_str(&import_text);
        text.push('\n');
        if needs_trailing_newline && insertion_point == 0 {
            // At the very beginning of the file, add extra newline to separate from code
        }

        Ok(vec![TextEdit {
            start: insertion_point,
            end: insertion_point,
            replacement: text,
            label: format!("add import from '{}'", self.source_module),
            priority: 0,
        }])
    }

    /// Format a complete import statement matching the file's conventions.
    fn format_full_import(&self, source: &str, _existing_specifiers: &[&str]) -> String {
        let quote = detect_quote_style(source);
        let semi = if detect_semicolons(source) { ";" } else { "" };
        let type_keyword = if self.type_only { "type " } else { "" };

        let mut parts = Vec::new();

        // Default import
        if let Some(ref default) = self.default_import {
            parts.push(default.clone());
        }

        // Named imports
        if !self.specifiers.is_empty() {
            let specs = self.specifiers.join(", ");
            parts.push(format!("{{ {} }}", specs));
        }

        let import_clause = parts.join(", ");
        format!(
            "import {}{} from {}{}{}{}",
            type_keyword, import_clause, quote, self.source_module, quote, semi
        )
    }
}

/// The remove_import operation.
pub struct RemoveImport {
    pub source_module: String,
    pub specifiers: Vec<String>, // empty = remove entire import
}

impl RemoveImport {
    pub fn new(source_module: String, specifiers: Vec<String>) -> Self {
        Self {
            source_module,
            specifiers,
        }
    }
}

impl Executable for RemoveImport {
    fn compute_edits(
        &self,
        source: &str,
        tree: &Tree,
    ) -> Result<Vec<TextEdit>, OperationError> {
        let root = tree.root_node();

        let import_node = find_import_from_source(&root, source, &self.source_module)
            .ok_or_else(|| OperationError::TargetNotFound {
                description: format!("No import from '{}' found", self.source_module),
            })?;

        if self.specifiers.is_empty() {
            // Remove entire import statement (including trailing newline)
            let end = import_node.end_byte();
            let end_with_newline = if source.as_bytes().get(end) == Some(&b'\n') {
                end + 1
            } else {
                end
            };
            return Ok(vec![TextEdit {
                start: import_node.start_byte(),
                end: end_with_newline,
                replacement: String::new(),
                label: format!("remove import from '{}'", self.source_module),
                priority: 0,
            }]);
        }

        // Remove specific specifiers
        let existing = extract_existing_specifiers(&import_node, source);
        let remaining: Vec<&&str> = existing
            .iter()
            .filter(|s| !self.specifiers.contains(&s.to_string()))
            .collect();

        if remaining.is_empty() {
            // All specifiers removed -- delete entire import
            let end = import_node.end_byte();
            let end_with_newline = if source.as_bytes().get(end) == Some(&b'\n') {
                end + 1
            } else {
                end
            };
            return Ok(vec![TextEdit {
                start: import_node.start_byte(),
                end: end_with_newline,
                replacement: String::new(),
                label: format!("remove import from '{}'", self.source_module),
                priority: 0,
            }]);
        }

        // Rewrite import with remaining specifiers
        let quote = detect_quote_style(source);
        let semi = if detect_semicolons(source) { ";" } else { "" };
        let specs = remaining
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let new_import = format!(
            "import {{ {} }} from {}{}{}{}",
            specs, quote, self.source_module, quote, semi
        );

        Ok(vec![TextEdit {
            start: import_node.start_byte(),
            end: import_node.end_byte(),
            replacement: new_import,
            label: format!("remove specifiers from import '{}'", self.source_module),
            priority: 0,
        }])
    }
}

// --- Helper functions ---

/// Find an import_statement node that imports from the given source module.
fn find_import_from_source<'a>(
    root: &'a Node<'a>,
    source: &str,
    module: &str,
) -> Option<Node<'a>> {
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return None;
    }

    loop {
        let node = cursor.node();
        if node.kind() == "import_statement" {
            if let Some(source_node) = node.child_by_field_name("source") {
                let text = &source[source_node.start_byte()..source_node.end_byte()];
                // Strip quotes
                let unquoted = text.trim_matches(|c| c == '\'' || c == '"');
                if unquoted == module {
                    return Some(node);
                }
            }
        }
        if !cursor.goto_next_sibling() {
            break;
        }
    }

    None
}

/// Extract named specifier strings from an import statement.
fn extract_existing_specifiers<'a>(import_node: &Node, source: &'a str) -> Vec<&'a str> {
    let mut specifiers = Vec::new();

    if let Some(clause) = find_child_by_kind(import_node, "import_clause") {
        if let Some(named) = find_child_by_kind(&clause, "named_imports") {
            let mut cursor = named.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "import_specifier" {
                        if let Some(name) = child.child_by_field_name("name") {
                            specifiers
                                .push(&source[name.start_byte()..name.end_byte()]);
                        }
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }

    specifiers
}

/// Check if an import has a default import clause.
fn has_default_import(import_node: &Node, _source: &str) -> bool {
    if let Some(clause) = find_child_by_kind(import_node, "import_clause") {
        // A default import is an `identifier` child of the import_clause
        let mut cursor = clause.walk();
        if cursor.goto_first_child() {
            loop {
                if cursor.node().kind() == "identifier" {
                    return true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    false
}

/// Find a direct child node by its kind.
fn find_child_by_kind<'a>(node: &'a Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    if !cursor.goto_first_child() {
        return None;
    }
    loop {
        if cursor.node().kind() == kind {
            return Some(cursor.node());
        }
        if !cursor.goto_next_sibling() {
            break;
        }
    }
    None
}

/// Find the byte offset where new imports should be inserted.
fn find_import_insertion_point(source: &str, tree: &Tree) -> usize {
    let root = tree.root_node();
    let mut last_import_end: Option<usize> = None;

    let mut cursor = root.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.kind() == "import_statement" {
                let end = node.end_byte();
                let line_end = source[end..]
                    .find('\n')
                    .map(|i| end + i + 1)
                    .unwrap_or(end);
                last_import_end = Some(line_end);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    last_import_end.unwrap_or_else(|| {
        if source.starts_with("#!") {
            source.find('\n').map(|i| i + 1).unwrap_or(0)
        } else {
            0
        }
    })
}

/// Detect quote style from existing imports.
fn detect_quote_style(source: &str) -> char {
    let single = source.matches("from '").count();
    let double = source.matches("from \"").count();
    if single >= double { '\'' } else { '"' }
}

/// Detect whether the file uses semicolons.
fn detect_semicolons(source: &str) -> bool {
    let with_semi = source
        .lines()
        .take(30)
        .filter(|l| l.trim_end().ends_with(';'))
        .count();
    with_semi > 0
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

    fn parse_tsx(source: &str) -> Tree {
        let mut parser = Parser::new();
        let lang = tree_sitter_typescript::LANGUAGE_TSX.into();
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

    // --- add_import tests ---

    #[test]
    fn test_add_new_import_to_file_with_imports() {
        let source = "import { useState } from 'react';\n\nconst App = () => {};";
        let tree = parse_ts(source);
        let op = AddImport::new(
            "./utils".to_string(),
            vec!["formatDate".to_string()],
            None,
            false,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("import { formatDate } from './utils';"));
        assert!(result.contains("import { useState } from 'react';"));
        // New import should be after existing imports
        let react_pos = result.find("import { useState }").unwrap();
        let utils_pos = result.find("import { formatDate }").unwrap();
        assert!(utils_pos > react_pos);
    }

    #[test]
    fn test_add_import_to_empty_file() {
        let source = "const foo = 1;";
        let tree = parse_ts(source);
        let op = AddImport::new(
            "react".to_string(),
            vec!["useState".to_string()],
            None,
            false,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.starts_with("import { useState } from 'react'"));
        assert!(result.contains("const foo = 1;"));
    }

    #[test]
    fn test_merge_into_existing_import() {
        let source = "import { useState } from 'react';\n";
        let tree = parse_ts(source);
        let op = AddImport::new(
            "react".to_string(),
            vec!["useEffect".to_string()],
            None,
            false,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("useState"));
        assert!(result.contains("useEffect"));
        // Should be on one line, merged
        let import_count = result.matches("import").count();
        assert_eq!(import_count, 1, "Should still be one import statement");
    }

    #[test]
    fn test_add_already_existing_specifier_is_noop() {
        let source = "import { useState } from 'react';\n";
        let tree = parse_ts(source);
        let op = AddImport::new(
            "react".to_string(),
            vec!["useState".to_string()],
            None,
            false,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        assert!(edits.is_empty(), "Should be no-op for existing specifier");
    }

    #[test]
    fn test_add_import_matches_double_quotes() {
        let source = "import { useState } from \"react\";\n";
        let tree = parse_ts(source);
        let op = AddImport::new(
            "./utils".to_string(),
            vec!["formatDate".to_string()],
            None,
            false,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("from \"./utils\""));
    }

    #[test]
    fn test_add_type_only_import() {
        let source = "import { useState } from 'react';\n";
        let tree = parse_ts(source);
        let op = AddImport::new(
            "./types".to_string(),
            vec!["User".to_string()],
            None,
            true,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("import type { User } from './types'"));
    }

    #[test]
    fn test_add_default_import() {
        let source = "import { useState } from 'react';\n";
        let tree = parse_ts(source);
        let op = AddImport::new(
            "react".to_string(),
            vec![],
            Some("React".to_string()),
            false,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        // Should have React as default import in a new/rewritten import
        assert!(result.contains("React"));
    }

    #[test]
    fn test_add_multiple_specifiers() {
        let source = "const foo = 1;\n";
        let tree = parse_ts(source);
        let op = AddImport::new(
            "react".to_string(),
            vec![
                "useState".to_string(),
                "useEffect".to_string(),
                "useCallback".to_string(),
            ],
            None,
            false,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("useState"));
        assert!(result.contains("useEffect"));
        assert!(result.contains("useCallback"));
    }

    // --- remove_import tests ---

    #[test]
    fn test_remove_entire_import() {
        let source = "import { useState } from 'react';\nimport { Button } from './Button';\n";
        let tree = parse_ts(source);
        let op = RemoveImport::new("react".to_string(), vec![]);
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(!result.contains("react"));
        assert!(result.contains("Button"));
    }

    #[test]
    fn test_remove_specific_specifier() {
        let source = "import { useState, useEffect } from 'react';\n";
        let tree = parse_ts(source);
        let op = RemoveImport::new("react".to_string(), vec!["useState".to_string()]);
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(!result.contains("useState"));
        assert!(result.contains("useEffect"));
        assert!(result.contains("react"));
    }

    #[test]
    fn test_remove_last_specifier_removes_import() {
        let source = "import { useState } from 'react';\nconst x = 1;\n";
        let tree = parse_ts(source);
        let op = RemoveImport::new("react".to_string(), vec!["useState".to_string()]);
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(!result.contains("import"));
        assert!(result.contains("const x = 1;"));
    }

    #[test]
    fn test_remove_nonexistent_import_errors() {
        let source = "import { useState } from 'react';\n";
        let tree = parse_ts(source);
        let op = RemoveImport::new("./nonexistent".to_string(), vec![]);
        let result = op.compute_edits(source, &tree);
        assert!(result.is_err());
    }

    // --- re-parse validation ---

    #[test]
    fn test_add_import_result_parses_cleanly() {
        let source = "import { useState } from 'react';\n\nexport function App() {\n  const [x, setX] = useState(0);\n  return <div>{x}</div>;\n}\n";
        let tree = parse_tsx(source);
        let op = AddImport::new(
            "react".to_string(),
            vec!["useEffect".to_string()],
            None,
            false,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);

        // Re-parse and verify no errors
        let tree2 = parse_tsx(&result);
        assert!(
            !tree2.root_node().has_error(),
            "Result has syntax errors:\n{}",
            result
        );
    }
}
