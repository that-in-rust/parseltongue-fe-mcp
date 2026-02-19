//! Operation vocabulary and execution trait.

pub mod rename_symbol;

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
    // Future operations will be added here as they are implemented.
    // AddImport { ... },
    // RemoveImport { ... },
    // AddProp { ... },
    // etc.
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
