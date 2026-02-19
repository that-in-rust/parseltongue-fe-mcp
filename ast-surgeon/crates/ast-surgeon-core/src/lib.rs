//! ast-surgeon-core: Pure computation library for AST-level code manipulation.
//!
//! This crate is language-agnostic -- it operates on tree-sitter Trees
//! and computes text edits. Language-specific intelligence lives in
//! `ast-surgeon-lang`.

pub mod edit;
pub mod format;
pub mod operations;
pub mod validate;

use edit::{EditSet, TextEdit};
use operations::{ChangeDescription, Executable, Operation, OperationError, OperationResult};
use tree_sitter::Tree;

/// Execute a list of operations on a source string with a pre-parsed tree.
///
/// All operations compute edits against the ORIGINAL source, then edits
/// are merged and applied in a single pass. The result is re-parsed
/// and verified.
pub fn execute_operations(
    source: &str,
    tree: &Tree,
    ops: &[Operation],
    language: &tree_sitter::Language,
) -> Result<OperationResult, OperationError> {
    if ops.is_empty() {
        return Ok(OperationResult {
            content: source.to_string(),
            changes: vec![],
            warnings: vec![],
        });
    }

    // Compute edits for each operation
    let mut all_edits: Vec<TextEdit> = Vec::new();
    let mut all_warnings: Vec<String> = Vec::new();

    for op in ops {
        let executable = operation_to_executable(op)?;
        let edits = executable.compute_edits(source, tree)?;
        all_edits.extend(edits);
    }

    if all_edits.is_empty() {
        return Ok(OperationResult {
            content: source.to_string(),
            changes: vec![],
            warnings: all_warnings,
        });
    }

    // Merge all edits into a single EditSet (detects overlaps)
    let edit_set = EditSet::new(all_edits, source.len())?;

    // Apply edits
    let new_source = edit_set.apply(source);

    // Collect change descriptions
    let changes: Vec<ChangeDescription> = edit_set
        .iter()
        .map(|e| {
            // Compute line/column in the new source (approximate -- based on original positions)
            let line = source[..e.start].chars().filter(|c| *c == '\n').count() + 1;
            let col = e.start
                - source[..e.start]
                    .rfind('\n')
                    .map(|i| i + 1)
                    .unwrap_or(0)
                + 1;
            ChangeDescription {
                kind: e.label.clone(),
                line,
                column: col,
                summary: e.label.clone(),
            }
        })
        .collect();

    // Verify the result parses cleanly
    match validate::verify_parse(&new_source, language) {
        Ok(_) => {}
        Err(validate::ValidationError::SyntaxErrors { errors, .. }) => {
            // This is the "should never happen" case -- our edits produced bad syntax.
            // Return the error with the broken result for debugging.
            return Err(OperationError::InvalidResult { errors });
        }
        Err(validate::ValidationError::ParseFailed) => {
            return Err(OperationError::InvalidResult { errors: vec![] });
        }
    }

    Ok(OperationResult {
        content: new_source,
        changes,
        warnings: all_warnings,
    })
}

/// Convert an Operation enum variant to a boxed Executable.
fn operation_to_executable(op: &Operation) -> Result<Box<dyn Executable>, OperationError> {
    match op {
        Operation::RenameSymbol {
            from, to, scope, ..
        } => Ok(Box::new(operations::rename_symbol::RenameSymbol::new(
            from.clone(),
            to.clone(),
            scope.clone(),
        ))),
    }
}
