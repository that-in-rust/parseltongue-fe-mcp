use std::path::{Path, PathBuf};

/// Marker files that indicate a project root.
const PROJECT_MARKERS: &[&str] = &[
    "package.json",
    "tsconfig.json",
    "biome.json",
    "deno.json",
];

/// Walk upward from `start` to find the nearest directory containing a project marker file.
/// Returns `None` if no marker is found before reaching the filesystem root.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        for marker in PROJECT_MARKERS {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        if !current.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_project_root_with_package_json() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        let sub = dir.path().join("src").join("components");
        fs::create_dir_all(&sub).unwrap();

        let root = find_project_root(&sub).unwrap();
        assert_eq!(root, dir.path());
    }

    #[test]
    fn test_find_project_root_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_project_root(dir.path()).is_none());
    }
}
