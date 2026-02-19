//! `rename_symbol` operation: rename an identifier across a file.
//!
//! Walks the entire CST looking for identifier nodes matching `from`,
//! replaces each with `to`. Skips string literals and comments.

use crate::edit::TextEdit;
use crate::operations::{Executable, OperationError};
use tree_sitter::Tree;

/// The rename_symbol operation.
pub struct RenameSymbol {
    pub from: String,
    pub to: String,
    pub scope: Option<String>,
}

impl RenameSymbol {
    pub fn new(from: String, to: String, scope: Option<String>) -> Self {
        Self { from, to, scope }
    }
}

impl Executable for RenameSymbol {
    fn compute_edits(
        &self,
        source: &str,
        tree: &Tree,
    ) -> Result<Vec<TextEdit>, OperationError> {
        if self.from.is_empty() || self.to.is_empty() {
            return Err(OperationError::InvalidParams {
                message: "from and to must be non-empty".to_string(),
            });
        }
        if self.from == self.to {
            return Ok(vec![]); // No-op
        }

        let root = tree.root_node();
        let mut edits = Vec::new();
        let mut warnings = Vec::new();

        collect_rename_edits(
            &root,
            source,
            &self.from,
            &self.to,
            &self.scope,
            &mut edits,
            &mut warnings,
        );

        if edits.is_empty() {
            return Err(OperationError::TargetNotFound {
                description: format!("No identifier '{}' found in file", self.from),
            });
        }

        Ok(edits)
    }
}

/// Recursively walk the tree and collect rename edits for identifier nodes.
fn collect_rename_edits(
    node: &tree_sitter::Node,
    source: &str,
    from: &str,
    to: &str,
    scope: &Option<String>,
    edits: &mut Vec<TextEdit>,
    _warnings: &mut Vec<String>,
) {
    let mut cursor = node.walk();

    loop {
        let current = cursor.node();

        if is_identifier_node(&current) {
            let text = &source[current.start_byte()..current.end_byte()];
            if text == from {
                // Check scope restriction
                if scope_matches(&current, source, scope) {
                    edits.push(TextEdit {
                        start: current.start_byte(),
                        end: current.end_byte(),
                        replacement: to.to_string(),
                        label: format!("rename {} -> {}", from, to),
                        priority: 0,
                    });
                }
            }
        }

        // Recurse into children (but skip string literals and comments)
        if should_descend(&current) && cursor.goto_first_child() {
            collect_rename_edits(&cursor.node(), source, from, to, scope, edits, _warnings);
            // Process remaining siblings at this level
            while cursor.goto_next_sibling() {
                collect_rename_edits(&cursor.node(), source, from, to, scope, edits, _warnings);
            }
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
}

/// Check if a node is an identifier that should be renamed.
fn is_identifier_node(node: &tree_sitter::Node) -> bool {
    matches!(
        node.kind(),
        "identifier"
            | "property_identifier"
            | "shorthand_property_identifier"
            | "shorthand_property_identifier_pattern"
            | "type_identifier"
    )
}

/// Check if we should descend into a node's children.
/// We skip string literals, template strings, and comment nodes.
fn should_descend(node: &tree_sitter::Node) -> bool {
    !matches!(
        node.kind(),
        "string"
            | "template_string"
            | "string_fragment"
            | "comment"
            | "line_comment"
            | "block_comment"
            | "regex"
            | "regex_pattern"
    )
}

/// Check if a node is within the specified scope.
///
/// If `scope` is None, all matches are in scope.
/// If `scope` is Some(name), only matches inside a function/class with that name.
fn scope_matches(
    node: &tree_sitter::Node,
    source: &str,
    scope: &Option<String>,
) -> bool {
    let scope_name = match scope {
        Some(s) => s,
        None => return true,
    };

    // Walk up the tree looking for a function/class declaration with the scope name
    let mut current = node.parent();
    while let Some(parent) = current {
        match parent.kind() {
            "function_declaration"
            | "method_definition"
            | "class_declaration"
            | "arrow_function"
            | "function" => {
                // Try to find the name child
                if let Some(name_node) = parent.child_by_field_name("name") {
                    let name = &source[name_node.start_byte()..name_node.end_byte()];
                    if name == scope_name {
                        return true;
                    }
                }
            }
            _ => {}
        }
        current = parent.parent();
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_typescript(source: &str) -> Tree {
        let mut parser = Parser::new();
        let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        parser.set_language(&language).unwrap();
        parser.parse(source, None).unwrap()
    }

    fn parse_tsx(source: &str) -> Tree {
        let mut parser = Parser::new();
        let language = tree_sitter_typescript::LANGUAGE_TSX.into();
        parser.set_language(&language).unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_basic_rename() {
        let source = "const useAuth = () => {};\nconst result = useAuth();";
        let tree = parse_typescript(source);
        let op = RenameSymbol::new("useAuth".into(), "useSession".into(), None);
        let edits = op.compute_edits(source, &tree).unwrap();
        assert_eq!(edits.len(), 2);

        let edit_set = crate::edit::EditSet::new(edits, source.len()).unwrap();
        let result = edit_set.apply(source);
        assert_eq!(
            result,
            "const useSession = () => {};\nconst result = useSession();"
        );
    }

    #[test]
    fn test_rename_skips_strings() {
        let source = "const useAuth = () => {};\nconst msg = \"useAuth is deprecated\";";
        let tree = parse_typescript(source);
        let op = RenameSymbol::new("useAuth".into(), "useSession".into(), None);
        let edits = op.compute_edits(source, &tree).unwrap();
        // Should only rename the declaration, not the string content
        assert_eq!(edits.len(), 1);

        let edit_set = crate::edit::EditSet::new(edits, source.len()).unwrap();
        let result = edit_set.apply(source);
        assert!(result.contains("useSession"));
        assert!(result.contains("\"useAuth is deprecated\""));
    }

    #[test]
    fn test_rename_not_found() {
        let source = "const foo = 1;";
        let tree = parse_typescript(source);
        let op = RenameSymbol::new("bar".into(), "baz".into(), None);
        let result = op.compute_edits(source, &tree);
        assert!(result.is_err());
        match result.unwrap_err() {
            OperationError::TargetNotFound { .. } => {}
            other => panic!("Expected TargetNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_rename_noop_same_name() {
        let source = "const foo = 1;";
        let tree = parse_typescript(source);
        let op = RenameSymbol::new("foo".into(), "foo".into(), None);
        let edits = op.compute_edits(source, &tree).unwrap();
        assert!(edits.is_empty());
    }

    #[test]
    fn test_rename_in_import() {
        let source = "import { useAuth } from './hooks';";
        let tree = parse_typescript(source);
        let op = RenameSymbol::new("useAuth".into(), "useSession".into(), None);
        let edits = op.compute_edits(source, &tree).unwrap();
        assert_eq!(edits.len(), 1);

        let edit_set = crate::edit::EditSet::new(edits, source.len()).unwrap();
        let result = edit_set.apply(source);
        assert_eq!(result, "import { useSession } from './hooks';");
    }

    #[test]
    fn test_rename_in_export() {
        let source = "export { useAuth } from './hooks';";
        let tree = parse_typescript(source);
        let op = RenameSymbol::new("useAuth".into(), "useSession".into(), None);
        let edits = op.compute_edits(source, &tree).unwrap();
        assert_eq!(edits.len(), 1);

        let edit_set = crate::edit::EditSet::new(edits, source.len()).unwrap();
        let result = edit_set.apply(source);
        assert_eq!(result, "export { useSession } from './hooks';");
    }

    #[test]
    fn test_rename_in_tsx_jsx() {
        let source = r#"import { Button } from './Button';
function App() {
  return <Button onClick={() => {}}>Click</Button>;
}"#;
        let tree = parse_tsx(source);
        let op = RenameSymbol::new("Button".into(), "PrimaryButton".into(), None);
        let edits = op.compute_edits(source, &tree).unwrap();
        // Should rename: import specifier, JSX opening tag, JSX closing tag
        assert!(edits.len() >= 3);

        let edit_set = crate::edit::EditSet::new(edits, source.len()).unwrap();
        let result = edit_set.apply(source);
        assert!(result.contains("import { PrimaryButton }"));
        assert!(result.contains("<PrimaryButton"));
        assert!(result.contains("</PrimaryButton>"));
    }

    #[test]
    fn test_rename_preserves_formatting() {
        let source = "// This is a comment about useAuth\nconst useAuth = () => {\n  // Internal logic\n  return { user: null };\n};";
        let tree = parse_typescript(source);
        let op = RenameSymbol::new("useAuth".into(), "useSession".into(), None);
        let edits = op.compute_edits(source, &tree).unwrap();

        let edit_set = crate::edit::EditSet::new(edits, source.len()).unwrap();
        let result = edit_set.apply(source);

        // Comment should be untouched (useAuth in comment not renamed)
        assert!(result.contains("// This is a comment about useAuth"));
        // Declaration should be renamed
        assert!(result.contains("const useSession = () => {"));
        // Internal comment preserved
        assert!(result.contains("  // Internal logic"));
    }

    #[test]
    fn test_rename_multiple_occurrences() {
        let source = "function useAuth() { return useAuth.cache; }\nuseAuth(); useAuth();";
        let tree = parse_typescript(source);
        let op = RenameSymbol::new("useAuth".into(), "useSession".into(), None);
        let edits = op.compute_edits(source, &tree).unwrap();

        // Should find all occurrences
        assert!(edits.len() >= 4);

        let edit_set = crate::edit::EditSet::new(edits, source.len()).unwrap();
        let result = edit_set.apply(source);

        // All occurrences renamed
        assert!(!result.contains("useAuth"));
        assert!(result.contains("useSession"));

        // Re-parse to verify syntax is still valid
        let tree2 = parse_typescript(&result);
        assert!(!tree2.root_node().has_error());
    }
}
