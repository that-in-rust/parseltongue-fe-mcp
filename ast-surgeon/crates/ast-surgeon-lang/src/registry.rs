//! Language registry: detect and load grammars.

use crate::{LangError, SupportedLanguage};
use std::path::Path;
use tree_sitter::{Language, Parser};

/// Create a parser configured for the given language.
pub fn parser_for_language(lang: SupportedLanguage) -> Parser {
    let mut parser = Parser::new();
    parser
        .set_language(&lang.ts_language())
        .expect("language version mismatch with tree-sitter");
    parser
}

/// Detect language from a file path.
pub fn detect_language(path: &str) -> Result<SupportedLanguage, LangError> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    SupportedLanguage::from_extension(ext)
}

/// Get the tree-sitter Language object for a language string.
pub fn get_language(lang_str: &str) -> Result<Language, LangError> {
    let lang = SupportedLanguage::from_str(lang_str)?;
    Ok(lang.ts_language())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_typescript() {
        assert_eq!(
            detect_language("src/hooks/useAuth.ts").unwrap(),
            SupportedLanguage::TypeScript
        );
    }

    #[test]
    fn test_detect_tsx() {
        assert_eq!(
            detect_language("src/components/App.tsx").unwrap(),
            SupportedLanguage::Tsx
        );
    }

    #[test]
    fn test_detect_javascript() {
        assert_eq!(
            detect_language("config.js").unwrap(),
            SupportedLanguage::JavaScript
        );
    }

    #[test]
    fn test_detect_jsx() {
        assert_eq!(
            detect_language("App.jsx").unwrap(),
            SupportedLanguage::Jsx
        );
    }

    #[test]
    fn test_detect_css() {
        assert_eq!(
            detect_language("styles.css").unwrap(),
            SupportedLanguage::Css
        );
    }

    #[test]
    fn test_detect_unsupported() {
        assert!(detect_language("data.json").is_err());
    }

    #[test]
    fn test_parser_for_typescript() {
        let parser = parser_for_language(SupportedLanguage::TypeScript);
        // Parser should be usable
        drop(parser);
    }

    #[test]
    fn test_parser_for_tsx() {
        let parser = parser_for_language(SupportedLanguage::Tsx);
        drop(parser);
    }
}
