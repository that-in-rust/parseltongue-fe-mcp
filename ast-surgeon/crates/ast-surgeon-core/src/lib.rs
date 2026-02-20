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
    let all_warnings: Vec<String> = Vec::new();

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
        Operation::AddImport {
            source,
            specifiers,
            default_import,
            type_only,
            ..
        } => Ok(Box::new(operations::imports::AddImport::new(
            source.clone(),
            specifiers.clone(),
            default_import.clone(),
            *type_only,
        ))),
        Operation::RemoveImport {
            source,
            specifiers,
            ..
        } => Ok(Box::new(operations::imports::RemoveImport::new(
            source.clone(),
            specifiers.clone(),
        ))),
        Operation::UpdateImportPaths {
            old_path,
            new_path,
            match_mode,
            ..
        } => {
            let mode = operations::update_paths::MatchMode::from_str(match_mode)?;
            Ok(Box::new(operations::update_paths::UpdateImportPaths::new(
                old_path.clone(),
                new_path.clone(),
                mode,
            )))
        }
        Operation::AddParameter {
            function_name,
            param_name,
            param_type,
            default_value,
            position,
            ..
        } => {
            let pos = operations::signature::ParamPosition::from_str(position)?;
            Ok(Box::new(operations::signature::AddParameter::new(
                function_name.clone(),
                param_name.clone(),
                param_type.clone(),
                default_value.clone(),
                pos,
            )))
        }
        Operation::RemoveParameter {
            function_name,
            param_name,
            ..
        } => Ok(Box::new(operations::signature::RemoveParameter::new(
            function_name.clone(),
            param_name.clone(),
        ))),
        Operation::MakeAsync {
            function_name, ..
        } => Ok(Box::new(operations::make_async::MakeAsync::new(
            function_name.clone(),
        ))),
        Operation::WrapInBlock {
            start_line,
            end_line,
            wrap_kind,
            condition,
            item,
            iterable,
            ..
        } => {
            let kind = match wrap_kind.as_str() {
                "if" => {
                    let cond = condition.clone().ok_or_else(|| OperationError::InvalidParams {
                        message: "wrap_in_block with kind 'if' requires 'condition'".to_string(),
                    })?;
                    operations::wrap::WrapKind::If { condition: cond }
                }
                "try_catch" => {
                    let param = condition.clone().unwrap_or_else(|| "error".to_string());
                    operations::wrap::WrapKind::TryCatch { catch_param: param }
                }
                "for_of" => {
                    let it = item.clone().ok_or_else(|| OperationError::InvalidParams {
                        message: "wrap_in_block with kind 'for_of' requires 'item'".to_string(),
                    })?;
                    let iter = iterable.clone().ok_or_else(|| OperationError::InvalidParams {
                        message: "wrap_in_block with kind 'for_of' requires 'iterable'".to_string(),
                    })?;
                    operations::wrap::WrapKind::ForOf { item: it, iterable: iter }
                }
                "block" => operations::wrap::WrapKind::Block,
                other => {
                    return Err(OperationError::InvalidParams {
                        message: format!(
                            "Invalid wrap_kind '{}', expected 'if', 'try_catch', 'for_of', or 'block'",
                            other
                        ),
                    })
                }
            };
            Ok(Box::new(operations::wrap::WrapInBlock::new(
                *start_line,
                *end_line,
                kind,
            )))
        }
        Operation::ExtractToVariable {
            expression,
            variable_name,
            var_kind,
            type_annotation,
            ..
        } => {
            let kind = operations::extract::VarKind::from_str(var_kind)?;
            Ok(Box::new(operations::extract::ExtractToVariable::new(
                expression.clone(),
                variable_name.clone(),
                kind,
                type_annotation.clone(),
            )))
        }
    }
}
