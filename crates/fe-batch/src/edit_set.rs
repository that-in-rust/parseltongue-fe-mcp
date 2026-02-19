use crate::error::BatchError;
use crate::types::{BatchInput, EditOperation, CreateOperation};
use fe_common::fs_utils::resolve_within_root;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A validated edit operation with resolved absolute path.
#[derive(Debug)]
pub struct ValidatedEdit {
    pub absolute_path: PathBuf,
    pub relative_path: String,
    pub change: EditChange,
}

/// The kind of change to apply to a file.
#[derive(Debug)]
pub enum EditChange {
    FullContent(String),
    AstOperations(Vec<crate::types::AstOperation>),
}

/// A validated create operation with resolved absolute path.
#[derive(Debug)]
pub struct ValidatedCreate {
    pub absolute_path: PathBuf,
    pub relative_path: String,
    pub content: String,
}

/// Validate all edits and creates from the input. Returns validated operations
/// or an error if any validation rule is violated.
pub fn validate_input(
    project_root: &Path,
    input: &BatchInput,
) -> Result<(Vec<ValidatedEdit>, Vec<ValidatedCreate>), BatchError> {
    let edits = input.edits.as_deref().unwrap_or(&[]);
    let creates = input.creates.as_deref().unwrap_or(&[]);

    // Rule: at least one edit or create
    if edits.is_empty() && creates.is_empty() {
        return Err(BatchError::EmptyTransaction);
    }

    let mut seen_paths = HashSet::new();
    let mut validated_edits = Vec::with_capacity(edits.len());
    let mut validated_creates = Vec::with_capacity(creates.len());

    // Validate edits
    for edit in edits {
        let validated = validate_edit(project_root, edit, &mut seen_paths)?;
        validated_edits.push(validated);
    }

    // Validate creates
    for create in creates {
        let validated = validate_create(project_root, create, &mut seen_paths)?;
        validated_creates.push(validated);
    }

    Ok((validated_edits, validated_creates))
}

fn validate_edit(
    project_root: &Path,
    edit: &EditOperation,
    seen_paths: &mut HashSet<String>,
) -> Result<ValidatedEdit, BatchError> {
    // Rule: exactly one of content or operations
    match (&edit.content, &edit.operations) {
        (Some(_), Some(_)) => return Err(BatchError::AmbiguousEdit(edit.file.clone())),
        (None, None) => return Err(BatchError::EmptyEdit(edit.file.clone())),
        _ => {}
    }

    // Rule: path must resolve within project root
    let absolute_path = resolve_within_root(project_root, &edit.file)
        .map_err(|_| BatchError::PathTraversal(PathBuf::from(&edit.file)))?;

    // Rule: no duplicate paths
    let canonical_key = edit.file.replace('\\', "/");
    if !seen_paths.insert(canonical_key.clone()) {
        return Err(BatchError::DuplicatePath(edit.file.clone()));
    }

    // Rule: file must exist for edits
    if !absolute_path.exists() {
        return Err(BatchError::FileNotFound(absolute_path));
    }

    let change = if let Some(content) = &edit.content {
        EditChange::FullContent(content.clone())
    } else if let Some(ops) = &edit.operations {
        EditChange::AstOperations(ops.clone())
    } else {
        unreachable!("already checked above");
    };

    Ok(ValidatedEdit {
        absolute_path,
        relative_path: edit.file.clone(),
        change,
    })
}

fn validate_create(
    project_root: &Path,
    create: &CreateOperation,
    seen_paths: &mut HashSet<String>,
) -> Result<ValidatedCreate, BatchError> {
    // Rule: path must resolve within project root
    let absolute_path = resolve_within_root(project_root, &create.file)
        .map_err(|_| BatchError::PathTraversal(PathBuf::from(&create.file)))?;

    // Rule: no duplicate paths
    let canonical_key = create.file.replace('\\', "/");
    if !seen_paths.insert(canonical_key.clone()) {
        return Err(BatchError::DuplicatePath(create.file.clone()));
    }

    // Rule: file must NOT already exist for creates
    if absolute_path.exists() {
        return Err(BatchError::FileAlreadyExists(absolute_path));
    }

    Ok(ValidatedCreate {
        absolute_path,
        relative_path: create.file.clone(),
        content: create.content.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_input(edits: Vec<EditOperation>, creates: Vec<CreateOperation>) -> BatchInput {
        BatchInput {
            edits: if edits.is_empty() { None } else { Some(edits) },
            creates: if creates.is_empty() { None } else { Some(creates) },
            verify: Some(false),
            rollback_on_failure: Some(true),
        }
    }

    fn edit_op(file: &str, content: &str) -> EditOperation {
        EditOperation {
            file: file.to_string(),
            content: Some(content.to_string()),
            operations: None,
        }
    }

    fn create_op(file: &str, content: &str) -> CreateOperation {
        CreateOperation {
            file: file.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn test_validate_edits_with_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.ts"), "original").unwrap();

        let input = make_input(vec![edit_op("file.ts", "new content")], vec![]);
        let (edits, creates) = validate_input(dir.path(), &input).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(creates.len(), 0);
        assert_eq!(edits[0].relative_path, "file.ts");
    }

    #[test]
    fn test_validate_edits_file_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let input = make_input(vec![edit_op("nonexistent.ts", "content")], vec![]);
        let err = validate_input(dir.path(), &input).unwrap_err();
        assert!(matches!(err, BatchError::FileNotFound(_)));
    }

    #[test]
    fn test_validate_creates_file_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("exists.ts"), "").unwrap();

        let input = make_input(vec![], vec![create_op("exists.ts", "content")]);
        let err = validate_input(dir.path(), &input).unwrap_err();
        assert!(matches!(err, BatchError::FileAlreadyExists(_)));
    }

    #[test]
    fn test_validate_path_traversal_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let input = make_input(
            vec![edit_op("../../../etc/passwd", "hacked")],
            vec![],
        );
        let err = validate_input(dir.path(), &input).unwrap_err();
        assert!(
            matches!(err, BatchError::PathTraversal(_) | BatchError::FileNotFound(_)),
            "Expected PathTraversal or FileNotFound, got: {err:?}"
        );
    }

    #[test]
    fn test_validate_duplicate_paths_rejected() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.ts"), "original").unwrap();

        let input = make_input(
            vec![
                edit_op("file.ts", "first"),
                edit_op("file.ts", "second"),
            ],
            vec![],
        );
        let err = validate_input(dir.path(), &input).unwrap_err();
        assert!(matches!(err, BatchError::DuplicatePath(_)));
    }

    #[test]
    fn test_validate_ambiguous_edit_rejected() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.ts"), "original").unwrap();

        let input = make_input(
            vec![EditOperation {
                file: "file.ts".to_string(),
                content: Some("content".to_string()),
                operations: Some(vec![]),
            }],
            vec![],
        );
        let err = validate_input(dir.path(), &input).unwrap_err();
        assert!(matches!(err, BatchError::AmbiguousEdit(_)));
    }

    #[test]
    fn test_validate_empty_transaction_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let input = make_input(vec![], vec![]);
        let err = validate_input(dir.path(), &input).unwrap_err();
        assert!(matches!(err, BatchError::EmptyTransaction));
    }

    #[test]
    fn test_validate_creates_with_nested_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let input = make_input(
            vec![],
            vec![create_op("src/components/deep/New.tsx", "content")],
        );
        let (_, creates) = validate_input(dir.path(), &input).unwrap();
        assert_eq!(creates.len(), 1);
        assert_eq!(creates[0].relative_path, "src/components/deep/New.tsx");
    }
}
