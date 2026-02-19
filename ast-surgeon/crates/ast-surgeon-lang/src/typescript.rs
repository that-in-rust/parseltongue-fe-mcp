//! TypeScript/TSX language support.
//!
//! Provides tree-sitter query patterns for TS/TSX-specific constructs:
//! imports, exports, JSX elements, hook calls, function declarations.

/// Query to find all import statements.
pub const IMPORTS_QUERY: &str = r#"
(import_statement
  source: (string) @source
) @import
"#;

/// Query to find all named import specifiers.
pub const IMPORT_SPECIFIERS_QUERY: &str = r#"
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @specifier)))
  source: (string) @source
) @import
"#;

/// Query to find all export statements.
pub const EXPORTS_QUERY: &str = r#"
(export_statement) @export
"#;

/// Query to find all function declarations.
pub const FUNCTION_DECLARATIONS_QUERY: &str = r#"
[
  (function_declaration
    name: (identifier) @name) @func
  (lexical_declaration
    (variable_declarator
      name: (identifier) @name
      value: (arrow_function)) @func)
]
"#;

/// Query to find React hook calls (use* pattern).
pub const HOOK_CALLS_QUERY: &str = r#"
(call_expression
  function: (identifier) @hook
  (#match? @hook "^use[A-Z]")
) @call
"#;

/// Query to find JSX opening elements.
pub const JSX_ELEMENTS_QUERY: &str = r#"
[
  (jsx_opening_element
    name: (_) @name) @opening
  (jsx_self_closing_element
    name: (_) @name) @self_closing
]
"#;

/// Query to find JSX props/attributes.
pub const JSX_ATTRIBUTES_QUERY: &str = r#"
(jsx_attribute
  (property_identifier) @prop_name
) @attribute
"#;

/// Find the byte offset where new imports should be inserted.
///
/// Returns the byte offset after the last existing import statement,
/// or 0 if no imports exist (after any shebang line).
pub fn import_insertion_point(source: &str, tree: &tree_sitter::Tree) -> usize {
    let root = tree.root_node();
    let mut last_import_end: Option<usize> = None;

    let mut cursor = root.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.kind() == "import_statement" {
                // Find the end of this import's line
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
        // Check for shebang
        if source.starts_with("#!") {
            source.find('\n').map(|i| i + 1).unwrap_or(0)
        } else {
            0
        }
    })
}

/// Detect the quote style used in imports (single or double).
pub fn detect_quote_style(source: &str) -> char {
    // Count single vs double quotes in import statements
    let single_count = source.matches("from '").count();
    let double_count = source.matches("from \"").count();

    if single_count >= double_count {
        '\''
    } else {
        '"'
    }
}

/// Detect whether the file uses semicolons.
pub fn detect_semicolons(source: &str) -> bool {
    // Check first 20 lines for semicolons at end
    let with_semi = source
        .lines()
        .take(20)
        .filter(|l| l.trim_end().ends_with(';'))
        .count();
    let without_semi = source
        .lines()
        .take(20)
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("/*")
                && !trimmed.ends_with('{')
                && !trimmed.ends_with('}')
                && !trimmed.ends_with(';')
        })
        .count();

    with_semi >= without_semi
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_ts(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        let lang = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        parser.set_language(&lang).unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_import_insertion_point_after_imports() {
        let source = "import { useState } from 'react';\nimport { Button } from './Button';\n\nconst App = () => {};";
        let tree = parse_ts(source);
        let point = import_insertion_point(source, &tree);
        // Should be after the second import's newline
        assert!(point > 0);
        assert_eq!(&source[point..point + 1], "\n");
    }

    #[test]
    fn test_import_insertion_point_no_imports() {
        let source = "const foo = 1;\nconst bar = 2;";
        let tree = parse_ts(source);
        let point = import_insertion_point(source, &tree);
        assert_eq!(point, 0);
    }

    #[test]
    fn test_detect_single_quotes() {
        let source = "import { a } from './a';\nimport { b } from './b';";
        assert_eq!(detect_quote_style(source), '\'');
    }

    #[test]
    fn test_detect_double_quotes() {
        let source = "import { a } from \"./a\";\nimport { b } from \"./b\";";
        assert_eq!(detect_quote_style(source), '"');
    }

    #[test]
    fn test_detect_semicolons_true() {
        let source = "import { a } from './a';\nconst x = 1;\nconst y = 2;";
        assert!(detect_semicolons(source));
    }
}
