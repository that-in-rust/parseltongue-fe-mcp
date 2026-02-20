use fe_batch::types::{BatchInput, CreateOperation, EditOperation};
use fe_batch::Transaction;
use std::fs;

fn make_input(edits: Vec<EditOperation>, creates: Vec<CreateOperation>) -> BatchInput {
    BatchInput {
        edits: if edits.is_empty() { None } else { Some(edits) },
        creates: if creates.is_empty() { None } else { Some(creates) },
        verify: Some(false),
        rollback_on_failure: Some(true),
    }
}

#[test]
fn test_rollback_preserves_original_content_exactly() {
    let dir = tempfile::tempdir().unwrap();
    // Include tricky content: trailing newline, special chars, etc.
    let original = "export const x = 1;\n// Special chars: é à ü ñ\n\n";
    fs::write(dir.path().join("file.ts"), original).unwrap();

    let input = make_input(
        vec![EditOperation {
            file: "file.ts".to_string(),
            content: Some("completely different".to_string()),
            operations: None,
        }],
        vec![],
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let applied = txn.stage().unwrap().apply().unwrap();

    // Verify the write happened
    assert_eq!(
        fs::read_to_string(dir.path().join("file.ts")).unwrap(),
        "completely different"
    );

    // Rollback
    let _rolled_back = applied.rollback().unwrap();
    // Must be byte-for-byte identical
    assert_eq!(fs::read_to_string(dir.path().join("file.ts")).unwrap(), original);
}

#[test]
fn test_rollback_after_create_removes_files_and_dirs() {
    let dir = tempfile::tempdir().unwrap();

    let input = make_input(
        vec![],
        vec![CreateOperation {
            file: "src/components/deep/NewComponent.tsx".to_string(),
            content: "export default function() {}".to_string(),
        }],
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let applied = txn.stage().unwrap().apply().unwrap();

    // File and dirs should exist
    assert!(dir.path().join("src/components/deep/NewComponent.tsx").exists());

    // Rollback
    let _rolled_back = applied.rollback().unwrap();
    assert!(!dir.path().join("src/components/deep/NewComponent.tsx").exists());
    // Empty parent dirs should be cleaned up
    assert!(!dir.path().join("src").exists());
}

#[test]
fn test_rollback_mixed_edits_and_creates() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("existing.ts"), "original content").unwrap();

    let input = make_input(
        vec![EditOperation {
            file: "existing.ts".to_string(),
            content: Some("modified content".to_string()),
            operations: None,
        }],
        vec![CreateOperation {
            file: "new_file.ts".to_string(),
            content: "new content".to_string(),
        }],
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let applied = txn.stage().unwrap().apply().unwrap();

    // Both changes should be applied
    assert_eq!(
        fs::read_to_string(dir.path().join("existing.ts")).unwrap(),
        "modified content"
    );
    assert!(dir.path().join("new_file.ts").exists());

    // Rollback
    let _rolled_back = applied.rollback().unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("existing.ts")).unwrap(),
        "original content"
    );
    assert!(!dir.path().join("new_file.ts").exists());
}

#[test]
#[cfg(unix)]
fn test_rollback_after_partial_apply_failure() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();

    // Create files: file_0 is writable, readonly dir has read+execute (can stat) but no write
    fs::write(dir.path().join("file_0.ts"), "original_0").unwrap();
    let readonly_dir = dir.path().join("readonly");
    fs::create_dir(&readonly_dir).unwrap();
    fs::write(readonly_dir.join("file.ts"), "original_readonly").unwrap();

    // Build the transaction while the file is still accessible
    let input = make_input(
        vec![
            EditOperation {
                file: "file_0.ts".to_string(),
                content: Some("modified_0".to_string()),
                operations: None,
            },
            EditOperation {
                file: "readonly/file.ts".to_string(),
                content: Some("modified_readonly".to_string()),
                operations: None,
            },
        ],
        vec![],
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let staged = txn.stage().unwrap();

    // Now make the directory read-only (0o555 = read+execute, no write)
    // This allows reading/stating files but prevents creating temp files for atomic_write
    fs::set_permissions(&readonly_dir, fs::Permissions::from_mode(0o555)).unwrap();

    let result = staged.apply();

    // The apply should fail because readonly dir prevents temp file creation
    assert!(result.is_err(), "Expected apply to fail, but it succeeded");

    // file_0 should be rolled back to original
    assert_eq!(
        fs::read_to_string(dir.path().join("file_0.ts")).unwrap(),
        "original_0"
    );

    // Cleanup: restore directory permissions so tempdir cleanup works
    fs::set_permissions(&readonly_dir, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn test_multiple_rollbacks_idempotent() {
    // Verify that the rollback result type prevents double-rollback at compile time.
    // This is a compile-time guarantee from the typestate pattern, so we just verify
    // the basic flow works.
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("file.ts"), "original").unwrap();

    let input = make_input(
        vec![EditOperation {
            file: "file.ts".to_string(),
            content: Some("modified".to_string()),
            operations: None,
        }],
        vec![],
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let applied = txn.stage().unwrap().apply().unwrap();
    let rolled_back = applied.rollback().unwrap();

    // After rollback, the Transaction<RolledBack> cannot call rollback() again
    // because the method only exists on Transaction<Applied>.
    // This is a compile-time guarantee, but we verify the file is restored.
    let result = rolled_back.into_result(None);
    assert!(result.rolled_back);
    assert_eq!(
        fs::read_to_string(dir.path().join("file.ts")).unwrap(),
        "original"
    );
}
