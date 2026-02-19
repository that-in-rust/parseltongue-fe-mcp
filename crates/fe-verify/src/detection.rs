use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct DetectedTools {
    pub linter: Option<LinterKind>,
    pub type_checker: Option<TypeCheckerKind>,
    pub test_runner: Option<TestRunnerKind>,
}

#[derive(Debug)]
pub enum LinterKind {
    ESLint { bin: PathBuf },
    Biome { bin: PathBuf },
}

#[derive(Debug)]
pub enum TypeCheckerKind {
    Tsc { bin: PathBuf },
}

#[derive(Debug)]
pub enum TestRunnerKind {
    Jest { bin: PathBuf },
    Vitest { bin: PathBuf },
}

/// Detect which verification tools are available in the project.
pub fn detect_tools(project_root: &Path) -> DetectedTools {
    let node_bin = project_root.join("node_modules").join(".bin");

    let linter = detect_linter(project_root, &node_bin);
    let type_checker = detect_type_checker(project_root, &node_bin);
    let test_runner = detect_test_runner(project_root, &node_bin);

    DetectedTools {
        linter,
        type_checker,
        test_runner,
    }
}

fn detect_linter(project_root: &Path, node_bin: &Path) -> Option<LinterKind> {
    // Check for Biome config
    if project_root.join("biome.json").exists() || project_root.join("biome.jsonc").exists() {
        if let Some(bin) = find_bin("biome", node_bin) {
            return Some(LinterKind::Biome { bin });
        }
    }

    // Check for ESLint config
    let eslint_configs = [
        "eslint.config.js",
        "eslint.config.mjs",
        "eslint.config.cjs",
        ".eslintrc.js",
        ".eslintrc.json",
        ".eslintrc.yml",
        ".eslintrc.yaml",
        ".eslintrc",
    ];
    if eslint_configs.iter().any(|c| project_root.join(c).exists()) {
        if let Some(bin) = find_bin("eslint", node_bin) {
            return Some(LinterKind::ESLint { bin });
        }
    }

    None
}

fn detect_type_checker(project_root: &Path, node_bin: &Path) -> Option<TypeCheckerKind> {
    if project_root.join("tsconfig.json").exists() {
        if let Some(bin) = find_bin("tsc", node_bin) {
            return Some(TypeCheckerKind::Tsc { bin });
        }
    }
    None
}

fn detect_test_runner(project_root: &Path, node_bin: &Path) -> Option<TestRunnerKind> {
    // Check for Vitest config
    let vitest_configs = [
        "vitest.config.ts",
        "vitest.config.js",
        "vitest.config.mts",
        "vitest.config.mjs",
    ];
    if vitest_configs.iter().any(|c| project_root.join(c).exists()) {
        if let Some(bin) = find_bin("vitest", node_bin) {
            return Some(TestRunnerKind::Vitest { bin });
        }
    }

    // Check for Jest config
    let jest_configs = [
        "jest.config.js",
        "jest.config.ts",
        "jest.config.mjs",
        "jest.config.cjs",
    ];
    if jest_configs.iter().any(|c| project_root.join(c).exists()) {
        if let Some(bin) = find_bin("jest", node_bin) {
            return Some(TestRunnerKind::Jest { bin });
        }
    }

    None
}

fn find_bin(name: &str, node_bin: &Path) -> Option<PathBuf> {
    // First check node_modules/.bin
    let local = node_bin.join(name);
    if local.exists() {
        return Some(local);
    }

    // Fall back to global PATH
    which::which(name).ok()
}
