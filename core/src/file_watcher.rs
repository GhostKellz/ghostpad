//! File system watcher for detecting external document modifications.

use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RemoveKind},
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

/// Events that can be detected by the file watcher.
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// File content was modified externally.
    Modified(PathBuf),
    /// File was deleted or moved away.
    Deleted(PathBuf),
}

/// Watches files for external modifications.
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    watched_paths: HashMap<PathBuf, u64>,
    event_rx: Receiver<Result<Event, notify::Error>>,
}

impl FileWatcher {
    /// Create a new file watcher.
    pub fn new() -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        Ok(Self {
            watcher,
            watched_paths: HashMap::new(),
            event_rx: rx,
        })
    }

    /// Start watching a file path for the given document ID.
    pub fn watch(&mut self, path: PathBuf, doc_id: u64) -> notify::Result<()> {
        if self.watched_paths.contains_key(&path) {
            return Ok(());
        }

        self.watcher.watch(&path, RecursiveMode::NonRecursive)?;
        self.watched_paths.insert(path, doc_id);
        Ok(())
    }

    /// Stop watching a file path.
    pub fn unwatch(&mut self, path: &PathBuf) -> notify::Result<()> {
        if self.watched_paths.remove(path).is_some() {
            self.watcher.unwatch(path)?;
        }
        Ok(())
    }

    /// Get the document ID associated with a path.
    pub fn doc_id_for_path(&self, path: &PathBuf) -> Option<u64> {
        self.watched_paths.get(path).copied()
    }

    /// Poll for pending file events.
    /// Returns a list of (document_id, event) pairs.
    pub fn poll_events(&self) -> Vec<(u64, FileEvent)> {
        let mut events = Vec::new();

        loop {
            match self.event_rx.try_recv() {
                Ok(Ok(event)) => {
                    if let Some((doc_id, file_event)) = self.process_event(event) {
                        events.push((doc_id, file_event));
                    }
                }
                Ok(Err(_)) => {
                    // Watcher error, skip
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        // Deduplicate events per document
        let mut seen: HashMap<u64, FileEvent> = HashMap::new();
        for (doc_id, event) in events {
            // Deleted takes priority over Modified
            match (&event, seen.get(&doc_id)) {
                (FileEvent::Deleted(_), _) => {
                    seen.insert(doc_id, event);
                }
                (FileEvent::Modified(_), None) => {
                    seen.insert(doc_id, event);
                }
                (FileEvent::Modified(_), Some(FileEvent::Modified(_))) => {
                    // Already have a modified event, skip
                }
                (FileEvent::Modified(_), Some(FileEvent::Deleted(_))) => {
                    // Deleted takes priority
                }
            }
        }

        seen.into_iter().collect()
    }

    fn process_event(&self, event: Event) -> Option<(u64, FileEvent)> {
        let path = event.paths.first()?;
        let doc_id = self.watched_paths.get(path).copied()?;

        match event.kind {
            EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Any)
            | EventKind::Modify(ModifyKind::Metadata(_)) => {
                Some((doc_id, FileEvent::Modified(path.clone())))
            }
            EventKind::Remove(RemoveKind::File) | EventKind::Remove(RemoveKind::Any) => {
                Some((doc_id, FileEvent::Deleted(path.clone())))
            }
            _ => None,
        }
    }
}
