//! Operation vocabulary and execution trait.

pub mod extract;
pub mod imports;
pub mod make_async;
pub mod rename_symbol;
pub mod signature;
pub mod update_paths;
pub mod wrap;

use crate::edit::TextEdit;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tree_sitter::Tree;

/// Errors that can occur during operation execution.
#[derive(Debug, Clone, Error, Serialize)]
pub enum OperationError {
    #[error("Target not found: {description}")]
    TargetNotFound { description: String },

    #[error("Ambiguous match: found {count} matches for {description}")]
    AmbiguousMatch {
        description: String,
        count: usize,
        locations: Vec<Location>,
    },

    #[error("Edit conflict: {0}")]
    EditConflict(#[from] crate::edit::EditConflict),

    #[error("Result has syntax errors")]
    InvalidResult {
        errors: Vec<crate::validate::SyntaxError>,
    },

    #[error("Source file has syntax errors")]
    SourceHasErrors {
        errors: Vec<crate::validate::SyntaxError>,
    },

    #[error("Unsupported language: {language}")]
    UnsupportedLanguage { language: String },

    #[error("Invalid operation parameters: {message}")]
    InvalidParams { message: String },
}

/// A source location for error reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub line: usize,
    pub column: usize,
    pub context: String,
}

/// Description of a change made by an operation.
#[derive(Debug, Clone, Serialize)]
pub struct ChangeDescription {
    /// e.g., "renamed identifier", "added import"
    pub kind: String,
    /// Line number in the NEW source (1-indexed).
    pub line: usize,
    /// Column in the NEW source (1-indexed).
    pub column: usize,
    /// Human-readable summary.
    pub summary: String,
}

/// The result of a successful operation.
#[derive(Debug, Clone, Serialize)]
pub struct OperationResult {
    /// The new file content.
    pub content: String,
    /// What changed.
    pub changes: Vec<ChangeDescription>,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
}

/// An operation request. This is the core enum defining the operation vocabulary.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    RenameSymbol {
        #[serde(default)]
        file: Option<String>,
        from: String,
        to: String,
        /// Restrict to a scope (function/class name). None = entire file.
        #[serde(default)]
        scope: Option<String>,
    },
    AddImport {
        #[serde(default)]
        file: Option<String>,
        /// Module path, e.g. "react" or "./utils".
        source: String,
        /// Named specifiers, e.g. ["useState", "useEffect"].
        #[serde(default)]
        specifiers: Vec<String>,
        /// Default import name, e.g. "React".
        #[serde(default)]
        default_import: Option<String>,
        /// If true, generates `import type { ... }`.
        #[serde(default)]
        type_only: bool,
    },
    RemoveImport {
        #[serde(default)]
        file: Option<String>,
        /// Module path to remove from.
        source: String,
        /// Specific specifiers to remove. Empty = remove entire import.
        #[serde(default)]
        specifiers: Vec<String>,
    },
    UpdateImportPaths {
        #[serde(default)]
        file: Option<String>,
        /// Old module path to match.
        old_path: String,
        /// New module path to replace with.
        new_path: String,
        /// "exact" or "prefix". Default: "exact".
        #[serde(default = "default_match_mode")]
        match_mode: String,
    },
    AddParameter {
        #[serde(default)]
        file: Option<String>,
        /// Name of the function to modify.
        function_name: String,
        /// Parameter name to add.
        param_name: String,
        /// Optional TypeScript type annotation.
        #[serde(default)]
        param_type: Option<String>,
        /// Optional default value expression.
        #[serde(default)]
        default_value: Option<String>,
        /// Position: "first", "last", or a 0-based index. Default: "last".
        #[serde(default = "default_position")]
        position: String,
    },
    RemoveParameter {
        #[serde(default)]
        file: Option<String>,
        /// Name of the function to modify.
        function_name: String,
        /// Parameter name to remove.
        param_name: String,
    },
    MakeAsync {
        #[serde(default)]
        file: Option<String>,
        /// Name of the function to make async.
        function_name: String,
    },
    WrapInBlock {
        #[serde(default)]
        file: Option<String>,
        /// First line to wrap (1-indexed).
        start_line: usize,
        /// Last line to wrap (1-indexed, inclusive).
        end_line: usize,
        /// Wrapper kind: "if", "try_catch", "for_of", "block".
        wrap_kind: String,
        /// Condition for if, catch param for try-catch, etc.
        #[serde(default)]
        condition: Option<String>,
        /// For for-of: iteration variable name.
        #[serde(default)]
        item: Option<String>,
        /// For for-of: iterable expression.
        #[serde(default)]
        iterable: Option<String>,
    },
    ExtractToVariable {
        #[serde(default)]
        file: Option<String>,
        /// The exact expression text to extract.
        expression: String,
        /// Name for the new variable.
        variable_name: String,
        /// "const" or "let". Default: "const".
        #[serde(default = "default_var_kind")]
        var_kind: String,
        /// Optional type annotation.
        #[serde(default)]
        type_annotation: Option<String>,
    },
}

fn default_var_kind() -> String {
    "const".to_string()
}

fn default_match_mode() -> String {
    "exact".to_string()
}

fn default_position() -> String {
    "last".to_string()
}

/// Trait for computing text edits from a parse tree.
///
/// Each operation implements this to produce edits.
pub trait Executable {
    /// Compute the text edits that implement this operation.
    fn compute_edits(
        &self,
        source: &str,
        tree: &Tree,
    ) -> Result<Vec<TextEdit>, OperationError>;
}
