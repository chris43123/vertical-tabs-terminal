VERDICT: PASS

## Blocking

## Notes
- Scope checked: the new pure `preview::selection` module, the hit-test/paint path in
  `ui/preview/code.rs`, the Copy/Copy all/Copy reference routing, the preview key
  handling in `app/input.rs`, markdown auto-scroll and the new `UiColors.selection`.
  The diff adds no dependencies, no `unsafe`, no process spawning and no file writes.
  The clipboard is written only after an explicit user action (button, menu, Ctrl+C).
  Nothing is logged.
- Panic review (hostile or odd file contents): `Text::new` always produces at least one
  line, so `t.lines[..]` and `t.line(..)` in `Selection` stay in bounds even for an empty
  file. `Pos::clamp` puts a position on a valid line and a char boundary, and it is
  monotone, so the `t.text[offset(a)..offset(b)]` slice in `Selection::text` can't run
  backwards or split a UTF-8 char. Every position stored in `Preview.selection` comes
  from `hit()` (`shown.start + char_indices` offsets), `word`, `lines` or `all`, so all
  of them are char boundaries. The cuts in `row_job` and the slice in `x_of` therefore
  stay valid, and the `byte > shown.end` / `a < x < b` guards cover the `MAX_LINE`
  truncation. A reload clears the selection, and Source/Rendered toggling clears it too,
  so no stale positions get painted against new text. Tests pass (`cargo test preview`:
  26 passed).
- Hardening, not reachable today: `hit()` in `src/ui/preview/code.rs` calls
  `p.clamp(g.visible.min, g.visible.max - vec2(0.0, 1.0))`. emath's `Pos2::clamp` uses
  `f32::clamp`, which panics when min > max, so this would panic if the viewport were
  shorter than 1 pt. egui's ScrollArea forces `min_scrolled_size` (64 pt) on its enabled
  axes, so the viewport is always at least 64 pt tall and this can't happen now. If that
  ever changes (custom `min_scrolled_height`, a different container), it becomes a crash.
  A `.max(g.visible.min)` on the upper bound would make it unconditionally safe.
- `Selection` positions are trusted when painting (`row_selection`/`x_of` don't clamp).
  That holds by construction today (see above). If a future change ever sets
  `preview.selection` from outside (say, a palette action with line numbers), clamp
  through `Pos::clamp` at that entry point.
- Copy reference writes the absolute path, unescaped. A hostile repo could hold a file
  named with an embedded newline, and pasting the reference into a shell without
  bracketed paste would run the rest of that line. The existing "Copy path" button
  already has this exposure (it is not new), and vtt's own paste path wraps in bracketed
  paste when the app asks for it. Worth a follow-up for both: quote or reject control
  characters in copied paths.
- Copied file text can contain raw control and escape bytes from a hostile file. That
  was already true with egui label selection. vtt's `encode_paste` strips ESC in
  bracketed mode, so pasting back into vtt is safe. Pasting into other terminals is
  their responsibility.
- DoS: the copy string (up to the 8 MB `MAX_TEXT`) is built only on a copy action, never
  per frame. Painting and hit-testing touch only the visible rows (plus at most one
  off-screen galley, and `hit` clamps to the visible rows anyway). Auto-scroll repaints
  only while the offset actually changes, so there is no idle loop.
