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
fn test_edit_three_files_atomically() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..3 {
        fs::write(dir.path().join(format!("file_{i}.ts")), format!("original_{i}")).unwrap();
    }

    let input = make_input(
        (0..3)
            .map(|i| EditOperation {
                file: format!("file_{i}.ts"),
                content: Some(format!("new_content_{i}")),
                operations: None,
            })
            .collect(),
        vec![],
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let committed = txn.stage().unwrap().apply().unwrap().commit();
    let result = committed.into_result(None);

    assert_eq!(result.files_modified.len(), 3);
    for i in 0..3 {
        assert_eq!(
            fs::read_to_string(dir.path().join(format!("file_{i}.ts"))).unwrap(),
            format!("new_content_{i}")
        );
    }
}

#[test]
fn test_create_three_files_atomically() {
    let dir = tempfile::tempdir().unwrap();
    // Create a package.json to make it look like a project root
    fs::write(dir.path().join("package.json"), "{}").unwrap();

    let input = make_input(
        vec![],
        (0..3)
            .map(|i| CreateOperation {
                file: format!("src/component_{i}.tsx"),
                content: format!("export const Component{i} = () => <div>{i}</div>;"),
            })
            .collect(),
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let committed = txn.stage().unwrap().apply().unwrap().commit();
    let result = committed.into_result(None);

    assert_eq!(result.files_created.len(), 3);
    for i in 0..3 {
        let content = fs::read_to_string(dir.path().join(format!("src/component_{i}.tsx"))).unwrap();
        assert!(content.contains(&format!("Component{i}")));
    }
}

#[test]
fn test_mixed_edits_and_creates() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("existing.ts"), "export const x = 1;").unwrap();

    let input = make_input(
        vec![EditOperation {
            file: "existing.ts".to_string(),
            content: Some("export const x = 2;".to_string()),
            operations: None,
        }],
        vec![
            CreateOperation {
                file: "new_a.ts".to_string(),
                content: "export const a = 'a';".to_string(),
            },
            CreateOperation {
                file: "new_b.ts".to_string(),
                content: "export const b = 'b';".to_string(),
            },
        ],
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let committed = txn.stage().unwrap().apply().unwrap().commit();
    let result = committed.into_result(None);

    assert_eq!(result.files_modified.len(), 1);
    assert_eq!(result.files_created.len(), 2);
    assert_eq!(
        fs::read_to_string(dir.path().join("existing.ts")).unwrap(),
        "export const x = 2;"
    );
    assert!(dir.path().join("new_a.ts").exists());
    assert!(dir.path().join("new_b.ts").exists());
}

#[test]
fn test_unicode_filenames() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Komponente.tsx"), "original").unwrap();

    let input = make_input(
        vec![EditOperation {
            file: "Komponente.tsx".to_string(),
            content: Some("aktualisiert".to_string()),
            operations: None,
        }],
        vec![CreateOperation {
            file: "Composant.tsx".to_string(),
            content: "nouveau".to_string(),
        }],
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let committed = txn.stage().unwrap().apply().unwrap().commit();
    let result = committed.into_result(None);

    assert_eq!(result.files_modified.len(), 1);
    assert_eq!(result.files_created.len(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("Komponente.tsx")).unwrap(),
        "aktualisiert"
    );
}

#[test]
fn test_large_file_handling() {
    let dir = tempfile::tempdir().unwrap();
    // Create a 1MB file (keeping it reasonable for CI)
    let large_content: String = "x".repeat(1_000_000);
    fs::write(dir.path().join("large.ts"), &large_content).unwrap();

    let new_large: String = "y".repeat(1_000_000);
    let input = make_input(
        vec![EditOperation {
            file: "large.ts".to_string(),
            content: Some(new_large.clone()),
            operations: None,
        }],
        vec![],
    );

    let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
    let committed = txn.stage().unwrap().apply().unwrap().commit();
    let result = committed.into_result(None);

    assert_eq!(result.files_modified.len(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("large.ts")).unwrap(),
        new_large
    );
}
