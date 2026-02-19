//! Formatting preservation: indentation inference and comment attachment.

use tree_sitter::Node;

/// Detected indentation style for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces(u8),
    Tabs,
}

/// Indentation context at a specific insertion point.
#[derive(Debug, Clone)]
pub struct IndentContext {
    /// The indentation style used in this file.
    pub style: IndentStyle,
    /// The exact whitespace prefix of the sibling line.
    /// We match this exactly rather than computing from depth.
    pub sibling_prefix: String,
}

/// Infer the indentation style used in a source file.
///
/// Scans the first 200 non-empty lines and votes on the most common
/// indentation delta between adjacent lines.
pub fn infer_indent_style(source: &str) -> IndentStyle {
    let mut space_counts = [0u32; 9]; // index = spaces per indent level
    let mut tab_count: u32 = 0;

    let lines: Vec<&str> = source.lines().take(200).collect();
    for pair in lines.windows(2) {
        let a_indent = count_leading_spaces(pair[0]);
        let b_indent = count_leading_spaces(pair[1]);

        if pair[1].starts_with('\t') {
            tab_count += 1;
        } else {
            let delta = b_indent.abs_diff(a_indent);
            if delta > 0 && delta <= 8 {
                space_counts[delta] += 1;
            }
        }
    }

    if tab_count > space_counts.iter().sum::<u32>() {
        IndentStyle::Tabs
    } else {
        let most_common = space_counts
            .iter()
            .enumerate()
            .skip(1)
            .max_by_key(|(_, &count)| count)
            .map(|(i, _)| i as u8)
            .unwrap_or(2);
        IndentStyle::Spaces(most_common)
    }
}

/// Extract the indentation context at a tree-sitter node.
///
/// Looks at the node's line to determine the exact whitespace prefix.
pub fn indent_context_at(source: &str, node: &Node) -> IndentContext {
    let style = infer_indent_style(source);
    let sibling_prefix = extract_line_prefix(source, node.start_byte());

    IndentContext {
        style,
        sibling_prefix,
    }
}

/// Extract the whitespace prefix of the line containing the given byte offset.
pub fn extract_line_prefix(source: &str, byte_offset: usize) -> String {
    let line_start = source[..byte_offset]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);

    source[line_start..]
        .chars()
        .take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r')
        .collect()
}

/// Indent a block of code to match the given prefix.
///
/// Each line (except the first) gets prefixed with `prefix`.
pub fn indent_code(code: &str, prefix: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let mut result = String::with_capacity(code.len() + lines.len() * prefix.len());
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push('\n');
            if !line.is_empty() {
                result.push_str(prefix);
            }
        }
        result.push_str(line);
    }
    // Preserve trailing newline if original had one
    if code.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// One indent level deeper than the given prefix.
pub fn indent_deeper(prefix: &str, style: &IndentStyle) -> String {
    match style {
        IndentStyle::Spaces(n) => format!("{}{}", prefix, " ".repeat(*n as usize)),
        IndentStyle::Tabs => format!("{}\t", prefix),
    }
}

fn count_leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// Span of a comment in source.
#[derive(Debug, Clone)]
pub struct CommentSpan {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// Comments attached to a specific AST node.
#[derive(Debug, Clone)]
pub struct AttachedComments {
    /// Comments on lines immediately above the node (in source order).
    pub leading: Vec<CommentSpan>,
    /// Comment on the same line after the node.
    pub trailing: Option<CommentSpan>,
}

/// Find comments attached to a node.
///
/// A comment is "leading" if it appears on immediately preceding lines
/// with no blank-line gap. A comment is "trailing" if it appears on the
/// same line after the node.
pub fn find_attached_comments(source: &str, node: &Node) -> AttachedComments {
    let mut leading = Vec::new();
    let mut trailing = None;

    // --- Trailing comment ---
    let node_end_row = node.end_position().row;
    let mut sibling = node.next_sibling();
    while let Some(sib) = sibling {
        if sib.start_position().row != node_end_row {
            break;
        }
        if is_comment_node(&sib) {
            trailing = Some(CommentSpan {
                start: sib.start_byte(),
                end: sib.end_byte(),
                text: source[sib.start_byte()..sib.end_byte()].to_string(),
            });
            break;
        }
        sibling = sib.next_sibling();
    }

    // --- Leading comments ---
    let mut prev = node.prev_sibling();
    while let Some(p) = prev {
        if is_comment_node(&p) {
            let gap = node.start_position().row.saturating_sub(p.end_position().row);
            if gap <= 1 {
                leading.push(CommentSpan {
                    start: p.start_byte(),
                    end: p.end_byte(),
                    text: source[p.start_byte()..p.end_byte()].to_string(),
                });
                prev = p.prev_sibling();
                continue;
            }
        }
        break;
    }

    leading.reverse();

    AttachedComments { leading, trailing }
}

fn is_comment_node(node: &Node) -> bool {
    matches!(node.kind(), "comment" | "line_comment" | "block_comment")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_2_spaces() {
        let source = "function foo() {\n  const x = 1;\n  if (true) {\n    return x;\n  }\n}";
        assert_eq!(infer_indent_style(source), IndentStyle::Spaces(2));
    }

    #[test]
    fn test_infer_4_spaces() {
        let source =
            "function foo() {\n    const x = 1;\n    if (true) {\n        return x;\n    }\n}";
        assert_eq!(infer_indent_style(source), IndentStyle::Spaces(4));
    }

    #[test]
    fn test_infer_tabs() {
        let source = "function foo() {\n\tconst x = 1;\n\tif (true) {\n\t\treturn x;\n\t}\n}";
        assert_eq!(infer_indent_style(source), IndentStyle::Tabs);
    }

    #[test]
    fn test_extract_line_prefix() {
        let source = "  const x = 1;\n    const y = 2;";
        // "    const y = 2;" starts at byte 15
        assert_eq!(extract_line_prefix(source, 15), "    ");
    }

    #[test]
    fn test_indent_code() {
        let code = "if (true) {\n  return 1;\n}";
        let result = indent_code(code, "  ");
        assert_eq!(result, "if (true) {\n    return 1;\n  }");
    }

    #[test]
    fn test_indent_deeper() {
        assert_eq!(indent_deeper("  ", &IndentStyle::Spaces(2)), "    ");
        assert_eq!(indent_deeper("\t", &IndentStyle::Tabs), "\t\t");
    }
}
