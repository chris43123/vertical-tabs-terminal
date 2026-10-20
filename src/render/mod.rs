//! GPU terminal rendering: a glyph atlas texture plus one textured mesh per pane.

pub mod font;
pub mod grid;

pub use font::Fonts;
pub use grid::{Palette, TermOpts, cell_at, grid_size_for, paint_terminal};
