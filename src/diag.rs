//! Problems worth showing the user (bad config, unknown theme, failed shell start, …).
//! They're printed to stderr and collected so the UI can show them in a banner.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static PROBLEMS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static QUIET: AtomicBool = AtomicBool::new(false);

/// Collect problems without printing them (`vtt config check` prints its own report).
pub fn set_quiet(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

pub fn warn(msg: impl Into<String>) {
    let msg = msg.into();
    if !QUIET.load(Ordering::Relaxed) {
        eprintln!("vtt: {msg}");
    }
    PROBLEMS.lock().unwrap().push(msg);
}

/// Take everything reported since the last call.
pub fn take() -> Vec<String> {
    std::mem::take(&mut *PROBLEMS.lock().unwrap())
}
