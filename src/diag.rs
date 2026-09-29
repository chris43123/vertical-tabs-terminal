//! Problems worth showing the user (bad config, unknown theme, failed shell start, …).
//! They're printed to stderr and collected so the UI can show them in a banner.

use std::sync::Mutex;

static PROBLEMS: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn warn(msg: impl Into<String>) {
    let msg = msg.into();
    eprintln!("vtt: {msg}");
    PROBLEMS.lock().unwrap().push(msg);
}

/// Take everything reported since the last call.
pub fn take() -> Vec<String> {
    std::mem::take(&mut *PROBLEMS.lock().unwrap())
}
