//! ast-surgeon-lang: Language-specific intelligence.
//!
//! This crate knows which tree-sitter grammars to use for which file types,
//! and provides language-specific query patterns and formatting rules.

pub mod registry;

#[cfg(feature = "typescript")]
pub mod typescript;

use thiserror::Error;
use tree_sitter::Language;

#[derive(Debug, Clone, Error)]
pub enum LangError {
    #[error("Unsupported language: {0}")]
    Unsupported(String),
}

/// Supported language identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupportedLanguage {
    TypeScript,
    Tsx,
    JavaScript,
    Jsx,
    Css,
}

impl SupportedLanguage {
    /// Parse a language string into a SupportedLanguage.
    pub fn from_str(s: &str) -> Result<Self, LangError> {
        match s.to_lowercase().as_str() {
            "typescript" | "ts" => Ok(Self::TypeScript),
            "tsx" => Ok(Self::Tsx),
            "javascript" | "js" => Ok(Self::JavaScript),
            "jsx" => Ok(Self::Jsx),
            "css" => Ok(Self::Css),
            other => Err(LangError::Unsupported(other.to_string())),
        }
    }

    /// Detect language from a file extension.
    pub fn from_extension(ext: &str) -> Result<Self, LangError> {
        match ext.trim_start_matches('.').to_lowercase().as_str() {
            "ts" => Ok(Self::TypeScript),
            "tsx" => Ok(Self::Tsx),
            "js" | "mjs" | "cjs" => Ok(Self::JavaScript),
            "jsx" => Ok(Self::Jsx),
            "css" => Ok(Self::Css),
            other => Err(LangError::Unsupported(other.to_string())),
        }
    }

    /// Get the tree-sitter Language for this language.
    pub fn ts_language(&self) -> Language {
        match self {
            #[cfg(feature = "typescript")]
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            #[cfg(feature = "typescript")]
            Self::Tsx | Self::Jsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            #[cfg(feature = "javascript")]
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            #[cfg(not(feature = "javascript"))]
            Self::JavaScript => {
                // Fall back to TypeScript parser for JS (JS is valid TS)
                #[cfg(feature = "typescript")]
                {
                    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
                }
                #[cfg(not(feature = "typescript"))]
                panic!("No JavaScript or TypeScript grammar available")
            }
            #[cfg(feature = "css")]
            Self::Css => tree_sitter_css::LANGUAGE.into(),
            #[allow(unreachable_patterns)]
            _ => panic!("Grammar not compiled for {:?}", self),
        }
    }
}
