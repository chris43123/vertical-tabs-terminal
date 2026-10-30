VERDICT: PASS

## Blocking

## Notes
- Scope this round: ce46efa "Keep selected text readable in terminals and previews"
  (`src/theme/chrome.rs` `SelectionColors`, the moved WCAG helpers, the new
  `selection_foreground` key in `src/theme/formats.rs` / `src/theme/mod.rs`, the one swap
  in `render::grid::paint_terminal`, and `ui/preview/code.rs` recoloring). There are no new
  dependencies, no `unsafe`, no process spawning, no file writes and no logging.
  `cargo test`: 104 passed.
- Rounds 1 and 2 had only non-blocking notes. They are unchanged and nothing here makes
  them worse: the `hit()` clamp hardening at `src/ui/preview/code.rs` still can't be
  reached, selection positions are still trusted when painting, and control characters in
  copied paths are still passed through as-is.
- Escape and OSC surface: programs in the terminal can't set the new key.
  `patch_from_osc` still fills only fg/bg/cursor/palette (`..Default::default()` leaves
  `selection_foreground` as `None`), and vtt doesn't act on OSC 17/19. Only the user's own
  theme file or config can set it. A hostile program could already recolor the palette
  through OSC 4/10/11. Even then, `SelectionColors::from_theme` ignores a
  `selection_foreground` below 3:1, and when no theme color reads it falls back to
  black/white. So OSC color games can't hide selected text any more than they could before.
- Parsing robustness: kitty `selection_foreground none` (valid in kitty) and other
  non-hex values are skipped by `parse_hex`, which slices nothing and only uses
  `from_str_radix` after a 6-byte length check, so multi-byte input just returns `None`. In
  TOML a bad value gives the existing `color()` error, not a panic.
- Panic review of the color math: every input is a `u8` channel, so `wcag_luminance` and
  `contrast_ratio` are finite and the denominator is at least 0.05. `ensure_contrast` now
  runs on all selected preview text, not only when `min_contrast > 1`. A hostile or odd
  `min_contrast` still can't panic it: `f32::clamp(1.0, 21.0)` has constant bounds, so it
  can't hit the min>max panic. NaN (valid TOML `nan`) passes through and fails every
  comparison, ending in `mix(1.0, to)`. inf clamps to 21.
- Cost: `SelectionColors::text` is a few `powf` per selected terminal cell or highlight
  section, only for visible cells and rows, and only on frames that are painted anyway.
  Preview selected rows are re-laid out each painted frame, but rows are capped at
  `MAX_LINE` (20,000 bytes) and only visible rows count. No new repaint or timer, so idle
  stays idle.
