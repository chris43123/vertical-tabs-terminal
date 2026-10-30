//! Polls a few files' modification times on a background thread and wakes the UI when one
//! changes. One `stat` per file per second costs nothing and needs no platform-specific APIs.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub struct Watcher {
    paths: Arc<Mutex<Vec<PathBuf>>>,
    rx: Receiver<Vec<PathBuf>>,
}

impl Watcher {
    pub fn spawn(paths: Vec<PathBuf>, ctx: eframe::egui::Context) -> Self {
        let paths = Arc::new(Mutex::new(paths));
        let (tx, rx) = channel();
        let shared = paths.clone();
        std::thread::Builder::new()
            .name("vtt config watcher".into())
            .spawn(move || {
                let stamp = |paths: &[PathBuf]| -> Vec<Option<SystemTime>> {
                    paths
                        .iter()
                        .map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
                        .collect()
                };
                let mut watched = shared.lock().unwrap().clone();
                let mut last = stamp(&watched);
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let current = shared.lock().unwrap().clone();
                    if current != watched {
                        // The watched set changed (e.g. a new theme file); start fresh.
                        watched = current;
                        last = stamp(&watched);
                        continue;
                    }
                    let now = stamp(&watched);
                    if now != last {
                        let changed = watched
                            .iter()
                            .zip(now.iter().zip(&last))
                            .filter(|(_, (a, b))| a != b)
                            .map(|(p, _)| p.clone())
                            .collect();
                        last = now;
                        if tx.send(changed).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                    }
                }
            })
            .expect("spawn watcher thread");
        Self { paths, rx }
    }

    /// Replace the set of watched files.
    pub fn set_paths(&self, paths: Vec<PathBuf>) {
        *self.paths.lock().unwrap() = paths;
    }

    /// The watched files that changed since the last call.
    pub fn changed(&self) -> Vec<PathBuf> {
        let mut changed: Vec<PathBuf> = Vec::new();
        while let Ok(paths) = self.rx.try_recv() {
            for p in paths {
                if !changed.contains(&p) {
                    changed.push(p);
                }
            }
        }
        changed
    }
}
