use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),
    #[error("Target does not exist")]
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatcherEvent {
    Modified(PathBuf),
    Removed(PathBuf),
}

/// A background filesystem watcher monitoring the active document.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    pub rx: Receiver<WatcherEvent>,
}

impl FileWatcher {
    pub fn watch(path: &Path) -> Result<Self, WatcherError> {
        let (tx, rx): (Sender<WatcherEvent>, Receiver<WatcherEvent>) = channel();
        let canonical_target = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let watch_dir = if canonical_target.is_file() {
            canonical_target.parent().unwrap_or(&canonical_target).to_path_buf()
        } else {
            canonical_target.clone()
        };

        let target_cmp = canonical_target.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    for p in event.paths {
                        let canon_p = p.canonicalize().unwrap_or(p);
                        if canon_p == target_cmp {
                            match event.kind {
                                EventKind::Modify(_) | EventKind::Create(_) => {
                                    let _ = tx.send(WatcherEvent::Modified(target_cmp.clone()));
                                }
                                EventKind::Remove(_) => {
                                    let _ = tx.send(WatcherEvent::Removed(target_cmp.clone()));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(100)),
        )?;

        watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;

        Ok(Self {
            _watcher: watcher,
            rx,
        })
    }
}
