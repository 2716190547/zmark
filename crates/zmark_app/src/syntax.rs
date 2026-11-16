#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmState {
    Waiting,
    Ready,
    InitFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleSyntaxFailure {
    pub source_line: usize,
    pub reason: &'static str,
}

/// One resumable parser per workspace; ordinary forward scrolling does not replay the file.
#[derive(Default)]
pub struct SourceSyntaxSession {
    context: Option<u64>,
    next: usize,
    parser: Option<zmark_core::syntax::SourceSyntax>,
}
impl SourceSyntaxSession {
    pub fn parse_visible(
        &mut self,
        blocks: &[zmark_core::ast::ParsedCodeBlock],
        wanted: &[zmark_core::syntax::CodeKey],
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Vec<(zmark_core::syntax::CodeKey, zmark_core::syntax::ParseResult)> {
        use zmark_core::syntax::{ParseResult, SourceSyntax};
        let Some(first) = blocks.iter().position(|block| wanted.contains(&block.key)) else {
            return Vec::new();
        };
        let last = blocks
            .iter()
            .rposition(|block| wanted.contains(&block.key))
            .unwrap();
        let context = blocks[first].key.source_context.map(|(context, _)| context);
        if context != self.context || first < self.next || self.parser.is_none() {
            self.context = context;
            self.next = 0;
            self.parser = SourceSyntax::new(blocks[first].key.language);
        }
        let mut results = Vec::new();
        for (index, block) in blocks.iter().enumerate().take(last + 1).skip(self.next) {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let result = self
                .parser
                .as_mut()
                .map_or(ParseResult::UnsupportedLanguage, |parser| {
                    parser.parse_chunk(&block.key.payload, Some(cancel))
                });
            if !result.is_ready() {
                // A preceding failure must also reach visible chunks, or admission would wait forever.
                results.extend(
                    wanted
                        .iter()
                        .filter(|key| !results.iter().any(|(done, _)| done == *key))
                        .map(|key| (key.clone(), result.clone()))
                        .collect::<Vec<_>>(),
                );
                self.parser = None;
                break;
            }
            self.next = index + 1;
            if wanted.contains(&block.key) {
                results.push((block.key.clone(), result));
            }
        }
        results
    }
}
