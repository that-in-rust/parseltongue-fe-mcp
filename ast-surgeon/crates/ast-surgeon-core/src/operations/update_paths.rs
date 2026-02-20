//! `update_import_paths` operation.
//!
//! Updates module specifier strings across a file after a file/directory move.
//! Handles `import`, `export`, and dynamic `import()` calls.

use crate::edit::TextEdit;
use crate::operations::{Executable, OperationError};
use tree_sitter::Tree;

/// How to match the old path against import specifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchMode {
    /// Exact match: specifier must equal `old_path` exactly.
    Exact,
    /// Prefix match: specifier starts with `old_path`.
    /// The matching prefix is replaced with `new_path`.
    Prefix,
}

impl MatchMode {
    pub fn from_str(s: &str) -> Result<Self, OperationError> {
        match s.to_lowercase().as_str() {
            "exact" => Ok(Self::Exact),
            "prefix" => Ok(Self::Prefix),
            other => Err(OperationError::InvalidParams {
                message: format!("Invalid match_mode '{}', expected 'exact' or 'prefix'", other),
            }),
        }
    }
}

/// The update_import_paths operation.
///
/// Finds all import/export/dynamic-import statements whose module specifier
/// matches `old_path` and replaces it with `new_path`.
pub struct UpdateImportPaths {
    pub old_path: String,
    pub new_path: String,
    pub match_mode: MatchMode,
}

impl UpdateImportPaths {
    pub fn new(old_path: String, new_path: String, match_mode: MatchMode) -> Self {
        Self {
            old_path,
            new_path,
            match_mode,
        }
    }
}

impl Executable for UpdateImportPaths {
    fn compute_edits(
        &self,
        source: &str,
        tree: &Tree,
    ) -> Result<Vec<TextEdit>, OperationError> {
        let root = tree.root_node();
        let mut edits = Vec::new();

        self.collect_string_edits(&root, source, &mut edits);

        if edits.is_empty() {
            return Err(OperationError::TargetNotFound {
                description: format!(
                    "No imports/exports with path '{}' found",
                    self.old_path
                ),
            });
        }

        Ok(edits)
    }
}

impl UpdateImportPaths {
    /// Walk the tree looking for string nodes inside import/export/dynamic-import
    /// that match the old path.
    fn collect_string_edits(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        edits: &mut Vec<TextEdit>,
    ) {
        // import_statement → has a "source" field (string)
        // export_statement → has a "source" field (string)
        // call_expression → if callee is "import", first argument is a string
        match node.kind() {
            "import_statement" | "export_statement" => {
                if let Some(source_node) = node.child_by_field_name("source") {
                    self.maybe_replace_string(&source_node, source, edits);
                }
            }
            "call_expression" => {
                // Dynamic import: import("./foo")
                if let Some(callee) = node.child_by_field_name("function") {
                    let callee_text = &source[callee.start_byte()..callee.end_byte()];
                    if callee_text == "import" {
                        if let Some(args) = node.child_by_field_name("arguments") {
                            // First argument
                            let mut cursor = args.walk();
                            if cursor.goto_first_child() {
                                loop {
                                    let child = cursor.node();
                                    if child.kind() == "string" {
                                        self.maybe_replace_string(&child, source, edits);
                                        break;
                                    }
                                    if !cursor.goto_next_sibling() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        // Recurse into children
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                self.collect_string_edits(&cursor.node(), source, edits);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// If the string node's content matches old_path, create an edit.
    fn maybe_replace_string(
        &self,
        string_node: &tree_sitter::Node,
        source: &str,
        edits: &mut Vec<TextEdit>,
    ) {
        let full_text = &source[string_node.start_byte()..string_node.end_byte()];
        if full_text.len() < 2 {
            return;
        }

        let quote = full_text.as_bytes()[0] as char;
        let unquoted = &full_text[1..full_text.len() - 1];

        let new_path = match self.match_mode {
            MatchMode::Exact => {
                if unquoted == self.old_path {
                    self.new_path.clone()
                } else {
                    return;
                }
            }
            MatchMode::Prefix => {
                if unquoted.starts_with(&self.old_path) {
                    let suffix = &unquoted[self.old_path.len()..];
                    format!("{}{}", self.new_path, suffix)
                } else {
                    return;
                }
            }
        };

        let replacement = format!("{}{}{}", quote, new_path, quote);
        edits.push(TextEdit {
            start: string_node.start_byte(),
            end: string_node.end_byte(),
            replacement,
            label: format!("update path '{}' → '{}'", unquoted, new_path),
            priority: 0,
        });
    }
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

    #[test]
    fn test_exact_match_single_import() {
        let source = "import { foo } from './utils';\n";
        let tree = parse_ts(source);
        let op = UpdateImportPaths::new(
            "./utils".to_string(),
            "./lib/utils".to_string(),
            MatchMode::Exact,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("from './lib/utils'"));
        assert!(!result.contains("from './utils'"));
    }

    #[test]
    fn test_exact_match_multiple_imports() {
        let source = "import { a } from './utils';\nimport { b } from './utils';\nimport { c } from './other';\n";
        let tree = parse_ts(source);
        let op = UpdateImportPaths::new(
            "./utils".to_string(),
            "./helpers".to_string(),
            MatchMode::Exact,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert_eq!(result.matches("'./helpers'").count(), 2);
        assert_eq!(result.matches("'./utils'").count(), 0);
        assert!(result.contains("'./other'"));
    }

    #[test]
    fn test_prefix_match() {
        let source = "import { a } from './components/Button';\nimport { b } from './components/Input';\nimport { c } from './utils';\n";
        let tree = parse_ts(source);
        let op = UpdateImportPaths::new(
            "./components".to_string(),
            "./ui/components".to_string(),
            MatchMode::Prefix,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("'./ui/components/Button'"));
        assert!(result.contains("'./ui/components/Input'"));
        assert!(result.contains("'./utils'")); // unchanged
    }

    #[test]
    fn test_export_statement() {
        let source = "export { default } from './old-module';\n";
        let tree = parse_ts(source);
        let op = UpdateImportPaths::new(
            "./old-module".to_string(),
            "./new-module".to_string(),
            MatchMode::Exact,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("from './new-module'"));
    }

    #[test]
    fn test_no_match_returns_error() {
        let source = "import { foo } from './utils';\n";
        let tree = parse_ts(source);
        let op = UpdateImportPaths::new(
            "./nonexistent".to_string(),
            "./whatever".to_string(),
            MatchMode::Exact,
        );
        let result = op.compute_edits(source, &tree);
        assert!(result.is_err());
    }

    #[test]
    fn test_preserves_quote_style() {
        let source = "import { foo } from \"./utils\";\n";
        let tree = parse_ts(source);
        let op = UpdateImportPaths::new(
            "./utils".to_string(),
            "./lib/utils".to_string(),
            MatchMode::Exact,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);
        assert!(result.contains("from \"./lib/utils\""));
    }

    #[test]
    fn test_result_parses_cleanly() {
        let source = "import { useState } from 'react';\nimport { Button } from './components/Button';\n\nexport function App() {\n  return <Button />;\n}\n";
        let tree = parse_tsx(source);
        let op = UpdateImportPaths::new(
            "./components/Button".to_string(),
            "./ui/Button".to_string(),
            MatchMode::Exact,
        );
        let edits = op.compute_edits(source, &tree).unwrap();
        let result = apply(source, edits);

        let tree2 = parse_tsx(&result);
        assert!(
            !tree2.root_node().has_error(),
            "Result has syntax errors:\n{}",
            result
        );
    }
}
