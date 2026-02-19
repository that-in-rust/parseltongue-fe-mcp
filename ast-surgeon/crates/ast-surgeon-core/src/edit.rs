//! Core text editing engine.
//!
//! Computes and applies non-overlapping byte-range replacements
//! using reverse-order application to avoid offset invalidation.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A single text replacement. Replaces bytes `[start..end)` with `replacement`.
/// `start` and `end` are byte offsets into the ORIGINAL source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    /// Byte offset of the first byte to replace (inclusive).
    pub start: usize,
    /// Byte offset past the last byte to replace (exclusive).
    pub end: usize,
    /// The replacement text.
    pub replacement: String,
    /// Human-readable label for diagnostics.
    pub label: String,
    /// Lower priority values appear earlier in output for same-position inserts.
    #[serde(default)]
    pub priority: i32,
}

/// Describes what went wrong when building an EditSet.
#[derive(Debug, Clone, Error, Serialize)]
pub enum EditConflict {
    #[error("Edits overlap: '{a_label}' [{a_start}..{a_end}) and '{b_label}' [{b_start}..{b_end})")]
    Overlapping {
        a_label: String,
        a_start: usize,
        a_end: usize,
        b_label: String,
        b_start: usize,
        b_end: usize,
    },
    #[error("Edit '{label}' byte range [{start}..{end}) exceeds source length {source_len}")]
    OutOfBounds {
        label: String,
        start: usize,
        end: usize,
        source_len: usize,
    },
}

/// A validated, non-overlapping set of edits for a single source string.
///
/// Edits are stored sorted by `(start, end, priority)` ascending.
/// The `apply` method processes them in REVERSE order so byte offsets
/// remain valid throughout.
#[derive(Debug, Clone)]
pub struct EditSet {
    edits: Vec<TextEdit>,
}

impl EditSet {
    /// Create a new EditSet, validating against the given source length.
    ///
    /// Returns `Err` if any edits overlap or are out of bounds.
    pub fn new(mut edits: Vec<TextEdit>, source_len: usize) -> Result<Self, EditConflict> {
        // Sort by (start, end, priority) ascending
        edits.sort_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then(a.end.cmp(&b.end))
                .then(a.priority.cmp(&b.priority))
        });

        // Check bounds
        for edit in &edits {
            if edit.start > edit.end || edit.end > source_len {
                return Err(EditConflict::OutOfBounds {
                    label: edit.label.clone(),
                    start: edit.start,
                    end: edit.end,
                    source_len,
                });
            }
        }

        // Check for overlaps: edit[i].end must be <= edit[i+1].start
        // (pure insertions at the same point are allowed: start == end)
        for pair in edits.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            // Two pure insertions at the same point are fine
            if a.start == a.end && b.start == b.end && a.start == b.start {
                continue;
            }
            if a.end > b.start {
                return Err(EditConflict::Overlapping {
                    a_label: a.label.clone(),
                    a_start: a.start,
                    a_end: a.end,
                    b_label: b.label.clone(),
                    b_start: b.start,
                    b_end: b.end,
                });
            }
        }

        Ok(Self { edits })
    }

    /// Apply all edits to `source` and return the new text.
    ///
    /// Processes edits in REVERSE byte-offset order so each edit's
    /// byte range is valid when applied (earlier bytes are unaffected
    /// by later replacements).
    pub fn apply(&self, source: &str) -> String {
        let mut result = source.to_string();
        for edit in self.edits.iter().rev() {
            result.replace_range(edit.start..edit.end, &edit.replacement);
        }
        result
    }

    /// Returns the number of edits.
    pub fn len(&self) -> usize {
        self.edits.len()
    }

    /// Returns true if there are no edits.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Returns an iterator over edits (sorted ascending by start offset).
    pub fn iter(&self) -> impl Iterator<Item = &TextEdit> {
        self.edits.iter()
    }
}

/// Merge multiple EditSets into one. Returns Err if any edits overlap.
pub fn merge_edit_sets(
    sets: Vec<EditSet>,
    source_len: usize,
) -> Result<EditSet, EditConflict> {
    let all_edits: Vec<TextEdit> = sets.into_iter().flat_map(|s| s.edits).collect();
    EditSet::new(all_edits, source_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_edit() {
        let source = "hello world";
        let edits = vec![TextEdit {
            start: 6,
            end: 11,
            replacement: "rust".to_string(),
            label: "replace world".to_string(),
            priority: 0,
        }];
        let edit_set = EditSet::new(edits, source.len()).unwrap();
        assert_eq!(edit_set.apply(source), "hello rust");
    }

    #[test]
    fn test_multiple_non_overlapping_edits() {
        let source = "aaa bbb ccc";
        let edits = vec![
            TextEdit {
                start: 0,
                end: 3,
                replacement: "xxx".to_string(),
                label: "first".to_string(),
                priority: 0,
            },
            TextEdit {
                start: 8,
                end: 11,
                replacement: "zzz".to_string(),
                label: "third".to_string(),
                priority: 0,
            },
        ];
        let edit_set = EditSet::new(edits, source.len()).unwrap();
        assert_eq!(edit_set.apply(source), "xxx bbb zzz");
    }

    #[test]
    fn test_insertion_at_same_point() {
        let source = "ab";
        let edits = vec![
            TextEdit {
                start: 1,
                end: 1,
                replacement: "X".to_string(),
                label: "insert1".to_string(),
                priority: 0,
            },
            TextEdit {
                start: 1,
                end: 1,
                replacement: "Y".to_string(),
                label: "insert2".to_string(),
                priority: 1,
            },
        ];
        let edit_set = EditSet::new(edits, source.len()).unwrap();
        // Priority 0 (X) applied after priority 1 (Y) in reverse,
        // so Y is inserted first, then X is inserted at the same position.
        // Reverse order: Y at pos 1, then X at pos 1 → "aXYb"
        let result = edit_set.apply(source);
        assert_eq!(result, "aYXb");
    }

    #[test]
    fn test_overlapping_edits_rejected() {
        let source = "hello world";
        let edits = vec![
            TextEdit {
                start: 3,
                end: 8,
                replacement: "X".to_string(),
                label: "edit1".to_string(),
                priority: 0,
            },
            TextEdit {
                start: 5,
                end: 10,
                replacement: "Y".to_string(),
                label: "edit2".to_string(),
                priority: 0,
            },
        ];
        let result = EditSet::new(edits, source.len());
        assert!(result.is_err());
        match result.unwrap_err() {
            EditConflict::Overlapping { .. } => {}
            other => panic!("Expected Overlapping, got {:?}", other),
        }
    }

    #[test]
    fn test_out_of_bounds_rejected() {
        let source = "hello";
        let edits = vec![TextEdit {
            start: 3,
            end: 10,
            replacement: "X".to_string(),
            label: "oob".to_string(),
            priority: 0,
        }];
        let result = EditSet::new(edits, source.len());
        assert!(result.is_err());
        match result.unwrap_err() {
            EditConflict::OutOfBounds { .. } => {}
            other => panic!("Expected OutOfBounds, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_edit_set() {
        let source = "unchanged";
        let edit_set = EditSet::new(vec![], source.len()).unwrap();
        assert_eq!(edit_set.apply(source), "unchanged");
        assert!(edit_set.is_empty());
    }

    #[test]
    fn test_deletion() {
        let source = "hello cruel world";
        let edits = vec![TextEdit {
            start: 5,
            end: 11,
            replacement: String::new(),
            label: "delete".to_string(),
            priority: 0,
        }];
        let edit_set = EditSet::new(edits, source.len()).unwrap();
        assert_eq!(edit_set.apply(source), "hello world");
    }

    #[test]
    fn test_merge_edit_sets() {
        let source = "aaa bbb ccc";
        let set1 = EditSet::new(
            vec![TextEdit {
                start: 0,
                end: 3,
                replacement: "xxx".to_string(),
                label: "a".to_string(),
                priority: 0,
            }],
            source.len(),
        )
        .unwrap();
        let set2 = EditSet::new(
            vec![TextEdit {
                start: 8,
                end: 11,
                replacement: "zzz".to_string(),
                label: "b".to_string(),
                priority: 0,
            }],
            source.len(),
        )
        .unwrap();
        let merged = merge_edit_sets(vec![set1, set2], source.len()).unwrap();
        assert_eq!(merged.apply(source), "xxx bbb zzz");
    }

    #[test]
    fn test_reverse_order_correctness() {
        // This test ensures that replacement with different-length strings
        // works correctly via reverse-order application.
        let source = "ab cd ef";
        let edits = vec![
            TextEdit {
                start: 0,
                end: 2,
                replacement: "LONGER".to_string(), // 2 bytes → 6 bytes
                label: "grow first".to_string(),
                priority: 0,
            },
            TextEdit {
                start: 3,
                end: 5,
                replacement: "X".to_string(), // 2 bytes → 1 byte
                label: "shrink middle".to_string(),
                priority: 0,
            },
            TextEdit {
                start: 6,
                end: 8,
                replacement: "YYY".to_string(), // 2 bytes → 3 bytes
                label: "grow last".to_string(),
                priority: 0,
            },
        ];
        let edit_set = EditSet::new(edits, source.len()).unwrap();
        assert_eq!(edit_set.apply(source), "LONGER X YYY");
    }
}
