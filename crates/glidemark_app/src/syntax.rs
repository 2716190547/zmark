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

