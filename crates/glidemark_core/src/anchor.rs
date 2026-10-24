use serde::{Deserialize, Serialize};

/// Source buffer line range coordinates mapped to a preview block element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAnchor {
    pub block_index: usize,
    pub source_line_start: usize,
    pub source_line_end: usize,
}

impl SourceAnchor {
    pub fn new(block_index: usize, start: usize, end: usize) -> Self {
        Self {
            block_index,
            source_line_start: start,
            source_line_end: end.max(start),
        }
    }
}
