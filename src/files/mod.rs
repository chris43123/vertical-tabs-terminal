//! The files panel's model: the lazily listed directory tree, git status and file search.

pub mod git;
pub mod search;
mod tree;

pub use tree::*;
