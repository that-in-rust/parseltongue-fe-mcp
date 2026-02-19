use std::path::{Path, PathBuf};

/// Resolve a relative path against a project root, ensuring the result
/// stays within the root (no `../` traversal escapes).
///
/// Returns the canonicalized absolute path on success, or an error message.
pub fn resolve_within_root(root: &Path, relative: &str) -> Result<PathBuf, String> {
    // Reject obviously malicious paths
    if relative.contains("..") {
        // Do a more precise check: normalize and verify
        let candidate = root.join(relative);
        let normalized = normalize_path(&candidate);
        if !normalized.starts_with(root) {
            return Err(format!(
                "Path '{}' escapes project root '{}'",
                relative,
                root.display()
            ));
        }
        return Ok(normalized);
    }

    let candidate = root.join(relative);
    Ok(normalize_path(&candidate))
}

/// Check if `path` is within `root` after normalization.
pub fn is_within_root(root: &Path, path: &Path) -> bool {
    let normalized = normalize_path(path);
    let normalized_root = normalize_path(root);
    normalized.starts_with(&normalized_root)
}

/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
/// Unlike `canonicalize()`, this does not require the path to exist.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => {
                components.push(other);
            }
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_normal_path() {
        let root = Path::new("/project");
        let result = resolve_within_root(root, "src/Component.tsx").unwrap();
        assert_eq!(result, PathBuf::from("/project/src/Component.tsx"));
    }

    #[test]
    fn test_resolve_rejects_traversal() {
        let root = Path::new("/project");
        let result = resolve_within_root(root, "../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_allows_internal_dotdot() {
        let root = Path::new("/project");
        // src/../lib/util.ts resolves to /project/lib/util.ts — still within root
        let result = resolve_within_root(root, "src/../lib/util.ts").unwrap();
        assert_eq!(result, PathBuf::from("/project/lib/util.ts"));
    }

    #[test]
    fn test_is_within_root() {
        let root = Path::new("/project");
        assert!(is_within_root(root, Path::new("/project/src/file.ts")));
        assert!(!is_within_root(root, Path::new("/other/file.ts")));
    }

    #[test]
    fn test_normalize_path() {
        let p = Path::new("/a/b/../c/./d");
        assert_eq!(normalize_path(p), PathBuf::from("/a/c/d"));
    }
}
