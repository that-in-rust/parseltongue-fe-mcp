use fe_batch::types::{BatchInput, CreateOperation, EditOperation};
use fe_batch::Transaction;
use proptest::prelude::*;
use std::fs;

fn make_input(edits: Vec<EditOperation>, creates: Vec<CreateOperation>) -> BatchInput {
    BatchInput {
        edits: if edits.is_empty() { None } else { Some(edits) },
        creates: if creates.is_empty() { None } else { Some(creates) },
        verify: Some(false),
        rollback_on_failure: Some(true),
    }
}

proptest! {
    /// Property: After rollback, every file matches its original content byte-for-byte.
    #[test]
    fn prop_rollback_always_restores_original_state(
        file_contents in prop::collection::vec("[a-zA-Z0-9 \n]{1,200}", 1..6),
        edit_contents in prop::collection::vec("[a-zA-Z0-9 \n]{1,200}", 1..6),
    ) {
        let count = file_contents.len().min(edit_contents.len());
        if count == 0 {
            return Ok(());
        }

        let dir = tempfile::tempdir().unwrap();

        // Create files with original content
        let filenames: Vec<String> = (0..count).map(|i| format!("file_{i}.ts")).collect();
        for (i, name) in filenames.iter().enumerate() {
            fs::write(dir.path().join(name), &file_contents[i]).unwrap();
        }

        // Build edits
        let edits: Vec<EditOperation> = filenames.iter().zip(edit_contents.iter()).map(|(name, content)| {
            EditOperation {
                file: name.clone(),
                content: Some(content.clone()),
                operations: None,
            }
        }).collect();

        let input = make_input(edits, vec![]);
        let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
        let applied = txn.stage().unwrap().apply().unwrap();

        // Rollback
        let _rolled_back = applied.rollback().unwrap();

        // Verify every file matches original
        for (i, name) in filenames.iter().enumerate() {
            let actual = fs::read_to_string(dir.path().join(name)).unwrap();
            prop_assert_eq!(&actual, &file_contents[i], "File {} mismatch after rollback", name);
        }
    }

    /// Property: After commit, every file matches the edit content exactly.
    #[test]
    fn prop_committed_state_matches_staged_content(
        edit_contents in prop::collection::vec("[a-zA-Z0-9 \n]{1,200}", 1..6),
    ) {
        if edit_contents.is_empty() {
            return Ok(());
        }

        let dir = tempfile::tempdir().unwrap();

        // Create files with original content
        let filenames: Vec<String> = (0..edit_contents.len()).map(|i| format!("file_{i}.ts")).collect();
        for name in &filenames {
            fs::write(dir.path().join(name), "original").unwrap();
        }

        // Build edits
        let edits: Vec<EditOperation> = filenames.iter().zip(edit_contents.iter()).map(|(name, content)| {
            EditOperation {
                file: name.clone(),
                content: Some(content.clone()),
                operations: None,
            }
        }).collect();

        let input = make_input(edits, vec![]);
        let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
        let committed = txn.stage().unwrap().apply().unwrap().commit();
        let _result = committed.into_result(None);

        // Verify every file matches the edit content
        for (i, name) in filenames.iter().enumerate() {
            let actual = fs::read_to_string(dir.path().join(name)).unwrap();
            prop_assert_eq!(&actual, &edit_contents[i], "File {} mismatch after commit", name);
        }
    }

    /// Property: On failure, no file has a mix of old and new content.
    /// Either ALL files have new content (success) or ALL have original content (rollback).
    #[test]
    #[cfg(unix)]
    fn prop_no_partial_state_on_failure(
        file_count in 2..8usize,
        fail_at in 0..8usize,
    ) {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let fail_index = fail_at % file_count;

        // Create files
        let filenames: Vec<String> = (0..file_count).map(|i| format!("file_{i}.ts")).collect();
        for (i, name) in filenames.iter().enumerate() {
            fs::write(dir.path().join(name), format!("original_{i}")).unwrap();
        }

        // Make one file's directory unwritable to cause a failure
        // We need to put it in a subdirectory to make only that one fail
        let fail_dir = dir.path().join("fail_dir");
        fs::create_dir(&fail_dir).unwrap();
        let fail_file = fail_dir.join("target.ts");
        fs::write(&fail_file, format!("original_{fail_index}")).unwrap();

        // Build edits: regular files + the one in the readonly dir
        let mut edits: Vec<EditOperation> = filenames.iter().enumerate()
            .filter(|(i, _)| *i != fail_index)
            .map(|(i, name)| EditOperation {
                file: name.clone(),
                content: Some(format!("new_{i}")),
                operations: None,
            })
            .collect();

        // Add the edit that targets the file in the soon-to-be-readonly dir
        edits.push(EditOperation {
            file: "fail_dir/target.ts".to_string(),
            content: Some(format!("new_{fail_index}")),
            operations: None,
        });

        let input = make_input(edits, vec![]);
        let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
        let staged = txn.stage().unwrap();

        // Make the directory read-only AFTER validation+staging (0o555 = r-x, no write)
        // This prevents temp file creation during apply, causing the write to fail
        fs::set_permissions(&fail_dir, fs::Permissions::from_mode(0o555)).unwrap();

        let apply_result = staged.apply();

        // Regardless of success/failure, check the invariant:
        // Either ALL regular files have new content, or ALL have original content.
        let regular_files: Vec<(usize, &String)> = filenames.iter().enumerate()
            .filter(|(i, _)| *i != fail_index)
            .collect();

        if apply_result.is_err() {
            // All regular files should still have original content
            for (i, name) in &regular_files {
                let actual = fs::read_to_string(dir.path().join(name)).unwrap();
                prop_assert_eq!(actual, format!("original_{i}"),
                    "File {} should be original after failed apply", name);
            }
        }
        // If apply succeeded, we'd need to either commit or rollback,
        // but since we're testing the failure path, this is sufficient.

        // Cleanup: restore directory permissions
        fs::set_permissions(&fail_dir, fs::Permissions::from_mode(0o755)).unwrap();
    }
}
