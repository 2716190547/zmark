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

/// Bidirectional mapping engine for sync scrolling between source editor lines
/// and preview virtual block indices.
#[derive(Debug, Clone, Default)]
pub struct SyncScrollMap {
    anchors: Vec<SourceAnchor>,
}

impl SyncScrollMap {
    pub fn new(mut anchors: Vec<SourceAnchor>) -> Self {
        anchors.sort_by_key(|a| a.source_line_start);
        Self { anchors }
    }

    pub fn anchors(&self) -> &[SourceAnchor] {
        &self.anchors
    }

    /// Translates source line number to target block index and normalized inner progress [0.0, 1.0].
    pub fn source_line_to_preview_progress(&self, line: usize) -> (usize, f32) {
        if self.anchors.is_empty() {
            return (0, 0.0);
        }

        let first = &self.anchors[0];
        if line <= first.source_line_start {
            return (first.block_index, 0.0);
        }

        let last = &self.anchors[self.anchors.len() - 1];
        if line >= last.source_line_end {
            return (last.block_index, 1.0);
        }

        for i in 0..self.anchors.len() {
            let curr = &self.anchors[i];
            if line >= curr.source_line_start && line <= curr.source_line_end {
                let progress = if curr.source_line_end == curr.source_line_start {
                    0.0
                } else {
                    (line - curr.source_line_start) as f32
                        / (curr.source_line_end - curr.source_line_start) as f32
                };
                return (curr.block_index, progress.clamp(0.0, 1.0));
            }

            if i + 1 < self.anchors.len() {
                let next = &self.anchors[i + 1];
                if line > curr.source_line_end && line < next.source_line_start {
                    // Blank lines between curr and next
                    let gap = next.source_line_start - curr.source_line_end;
                    let offset = line - curr.source_line_end;
                    let progress = offset as f32 / gap as f32;
                    return (curr.block_index, progress.clamp(0.0, 1.0));
                }
            }
        }

        (last.block_index, 1.0)
    }

    /// Translates preview block index and inner progress back to estimated source line.
    pub fn preview_to_source_line(&self, block_index: usize, progress: f32) -> usize {
        if self.anchors.is_empty() {
            return 1;
        }

        let anchor = self
            .anchors
            .iter()
            .find(|a| a.block_index == block_index)
            .unwrap_or_else(|| &self.anchors[self.anchors.len() - 1]);

        let p = progress.clamp(0.0, 1.0);
        let span = (anchor.source_line_end.saturating_sub(anchor.source_line_start)) as f32;
        anchor.source_line_start + (span * p).round() as usize
    }
}
