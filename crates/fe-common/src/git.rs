use std::path::{Path, PathBuf};
use std::process::Command;

/// Get files changed since last commit (staged + unstaged + untracked).
/// Returns paths relative to the project root.
pub fn changed_files(project_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    // Staged + unstaged modifications
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(project_root)
        .output()
        .map_err(|e| format!("Failed to run git diff: {e}"))?;

    if output.status.success() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if !line.is_empty() {
                files.push(PathBuf::from(line));
            }
        }
    }

    // Untracked files
    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(project_root)
        .output()
        .map_err(|e| format!("Failed to run git ls-files: {e}"))?;

    if output.status.success() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if !line.is_empty() {
                let p = PathBuf::from(line);
                if !files.contains(&p) {
                    files.push(p);
                }
            }
        }
    }

    Ok(files)
}

/// Filter a file list to only frontend-relevant files.
pub fn filter_frontend_files(files: &[PathBuf]) -> Vec<PathBuf> {
    let extensions = ["ts", "tsx", "js", "jsx", "vue", "svelte", "css", "scss"];
    files
        .iter()
        .filter(|f| {
            f.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| extensions.contains(&ext))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_frontend_files() {
        let files = vec![
            PathBuf::from("src/App.tsx"),
            PathBuf::from("src/utils.ts"),
            PathBuf::from("README.md"),
            PathBuf::from("Cargo.toml"),
            PathBuf::from("src/styles.css"),
            PathBuf::from("package.json"),
        ];
        let filtered = filter_frontend_files(&files);
        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains(&PathBuf::from("src/App.tsx")));
        assert!(filtered.contains(&PathBuf::from("src/utils.ts")));
        assert!(filtered.contains(&PathBuf::from("src/styles.css")));
    }
}
