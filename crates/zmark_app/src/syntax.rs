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

// Oniguruma capture regions are not Send. Keep the resumable parser on one
// thread; only immutable requests and owned token results cross the channel.
#[cfg(target_os = "macos")]
#[derive(Default)]
pub struct SourceSyntaxWorker {
    sender: std::sync::OnceLock<Option<std::sync::mpsc::Sender<SourceSyntaxJob>>>,
}
#[cfg(target_os = "macos")]
struct SourceSyntaxJob {
    blocks: std::sync::Arc<Vec<zmark_core::ast::ParsedCodeBlock>>,
    wanted: Vec<zmark_core::syntax::CodeKey>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    reply: std::sync::mpsc::Sender<
        Vec<(zmark_core::syntax::CodeKey, zmark_core::syntax::ParseResult)>,
    >,
}
#[cfg(target_os = "macos")]
impl SourceSyntaxWorker {
    pub fn parse_visible(
        &self,
        blocks: std::sync::Arc<Vec<zmark_core::ast::ParsedCodeBlock>>,
        wanted: Vec<zmark_core::syntax::CodeKey>,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Vec<(zmark_core::syntax::CodeKey, zmark_core::syntax::ParseResult)> {
        let sender = self.sender.get_or_init(|| {
            let (sender, receiver) = std::sync::mpsc::channel::<SourceSyntaxJob>();
            std::thread::Builder::new()
                .name("zmark-source-syntax".into())
                .spawn(move || {
                    let mut session = SourceSyntaxSession::default();
                    for job in receiver {
                        let results = session.parse_visible(&job.blocks, &job.wanted, &job.cancel);
                        let _ = job.reply.send(results);
                    }
                })
                .ok()
                .map(|_| sender)
        });
        let failure = || {
            wanted
                .iter()
                .cloned()
                .map(|key| (key, zmark_core::syntax::ParseResult::InitFailed))
                .collect()
        };
        let Some(sender) = sender else {
            return failure();
        };
        let (reply, receiver) = std::sync::mpsc::channel();
        if sender
            .send(SourceSyntaxJob {
                blocks,
                wanted: wanted.clone(),
                cancel,
                reply,
            })
            .is_err()
        {
            return failure();
        }
        receiver.recv().unwrap_or_else(|_| failure())
    }
}
