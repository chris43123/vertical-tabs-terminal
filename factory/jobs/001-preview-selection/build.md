# Build log: 001-preview-selection

## Initial build
Commits: 4645755 Select and copy anything in a preview, however long
Checks: fmt ✓  clippy ✓  test ✓ (100 passed)

### Acceptance criteria
1. ✓ `Selection::text` is exact for single-line, multi-line, reversed and multi-byte ranges. Test: `preview::selection::tests::text_of_single_multi_reversed_and_multibyte_ranges`.
2. ✓ Word, line and line-range expansion, select-all and `reference` (`a.md:3`, `a.md:3-9`). Test: `preview::selection::tests::word_line_range_all_and_reference`.
3. ✓ `autoscroll_step` is 0 inside the body, grows with overshoot and is capped (plus `autoscroll_axis` signs). Test: `preview::selection::tests::autoscroll_is_zero_inside_grows_and_caps`.
4. ✓ A file reload clears the selection to `None`. Tests: `preview::tests::reload_clears_the_selection` and `preview::tests::rendered_markdown_has_no_source_selection`.
5. ? 3000-line file, gutter line 1 then Shift+click line 3000, then Ctrl+C. Not run by hand (no interactive display in the build agent). By the code path: the gutter press calls `Selection::lines(0,0)`, the Shift+press calls `Selection::lines(anchor.line=0, 2999)`, then `Event::Copy` goes to `copy_selection` → `Preview::selected_text`. The pure parts are unit-tested. Needs a manual check.
6. ~ Dragging past the edge scrolls and extends the selection. Code views (wrap on and off): implemented. `CodeView` reports `response.dragged()`, and `ui::preview::autoscroll` moves the ScrollArea's stored offset and repaints only while the offset actually changes. The head comes from the pointer clamped to the visible rows. **Rendered markdown: auto-scroll is implemented, but the selection does not survive long distances.** egui's `LabelSelectionState` drops the selection as soon as the label holding its anchor is no longer visible (egui only processes visible labels: `label.rs` `is_rect_visible`, then `on_end_pass` "didn't see both cursors"). The scroll works and the selection extends while the anchor is on screen; after that egui clears it. Fixing this needs a custom selection model for rendered markdown, which the spec lists as out of scope. The workaround is documented in the README: Source plus line numbers, or toolbar Copy. Needs a manual check either way.
7. ? Toolbar Copy with no selection copies the whole file (markdown source when rendered). `Action::Copy` falls back to `text.text`. Not run by hand.
8. ? Ctrl+C in a focused terminal behaves as before. The terminal branch of `copy_selection` is unchanged. The preview fallback runs only when the focused tab has no session, and `preview_key` returns false for non-preview tabs. Not run by hand.
9. ? Idle CPU ~0%. The only new `request_repaint` is in `autoscroll`, and it fires only while a drag is active and the offset changed. No timers and no polling. Not measured.
10. ✓ fmt, clippy (`-D warnings`) and tests pass. README is updated: Features, a keybinding table row, and a "Copying" block under Files and previews. The help window has a new "Previews" section (`ui/help.rs` `previews()`).

### Notes for reviewers
- Code rows are now painted directly as galleys (`painter.galley`) instead of `Label`s. This gives the same visual result with no egui label selection in code views, so egui's `Event::Copy` handling and ours can't both fire there.
- Toolbar Copy in *rendered* markdown acts on the press, not the click. egui clears its label selection on any press elsewhere, so on that frame we push `Event::Copy` for egui's labels when it has a selection. Otherwise we copy the source.
- `UiColors.selection` comes from `Theme.selection`, falling back to a bg/fg blend if it is nearly equal to the background. With `min_contrast > 1`, selected text is recolored with `render::ensure_contrast` (now `pub`) against the selection color. Test: `ui::preview::code::tests::selected_text_keeps_minimum_contrast`, plus `theme::chrome::tests::selection_comes_from_the_theme_and_stays_visible`.
- Ctrl+A in rendered markdown does nothing, because the source selection isn't visible there. Toggling Source/Rendered clears the source selection.
- Copy reference uses the absolute path. With no selection, it uses the line that was right-clicked.
- Open question from the spec (a palette action for Copy reference) was left out.

## Human feedback (human.md, 2026-10-20T04:59:11Z)
Commits: 12c0bf5 Confirm copies with a short toast
Checks: fmt ✓  clippy ✓  test ✓ (102 passed)

### Acceptance criteria
1-4. ✓ Unchanged; the tests still pass.
5-9. ? Still manual, same as the initial build. The toast adds one `request_repaint_after` while it is shown and a per-frame repaint only during its 250 ms fade (none with `reduce_motion`), then goes idle (criterion 9).
10. ✓ fmt, clippy and test pass. README "Copying" now mentions the confirmation.

### Review responses
- human "feedback for when the copy happens, a toast perhaps?": done in 12c0bf5. New `src/ui/toast.rs`: a non-interactive note at the bottom-center of the pane the copy came from, or of the window for the files panel. It reads "Copied 3,000 lines" / "Copied 42 characters" for selections, "Copied the whole file (N lines)" or "Copied the markdown source (N lines)" for Copy / Copy all, "Copied a.md:3-9" for Copy reference, and "Copied the path". If the clipboard write fails it reads "Couldn't copy: the clipboard isn't available", drawn in the danger color. Rendered markdown's own egui selection gets "Copied the selection" from both the toolbar and Ctrl+C. It holds for 1.6 s, then fades over 0.25 s (no fade with `reduce_motion`). Colors come from `UiColors`. Tests: `ui::toast::tests::summarizes_what_was_copied` and `ui::toast::tests::holds_then_fades_then_goes`.
- Scope: terminal copies (Ctrl+C, Ctrl+Shift+C, right-click) get no toast, so terminal behavior stays exactly as before (criterion 8). If the human wants it there too, it's one call per site (`copy_with_toast`).

## Human feedback (human.md, 2026-10-20T05:02:24Z and 05:03:54Z)
Commits: ce46efa Keep selected text readable in terminals and previews
Checks: fmt ✓  clippy ✓  test ✓ (104 passed)

### Acceptance criteria
1-4. ✓ Unchanged; the tests still pass.
5-9. ? Still manual. Painting a selection does no extra repaints; selected preview rows are re-laid out with the new colors, but only the visible ones and only when they change (egui caches galleys).
10. ✓ fmt, clippy and test pass. README (theme keys, kitty keys, a new paragraph on readable selections, the matugen template) and `config.example.toml` (the `[colors]` comment, mirrored in README) are updated.

### Review responses
- human "selected text has almost no contrast… swap the font color?" + "include terminal as well": done in ce46efa.
  - New `theme::SelectionColors` (in `src/theme/chrome.rs`), used by both the terminal grid (`render::grid`, where selected cells take `palette.selection.text(fg)`) and the preview code view.
  - Rule: if the theme sets `selection_foreground` and it reads on the selection (≥ 3:1), all selected text uses it, as kitty does. Otherwise each color is kept when it reaches 4.5:1 on the selection, and swapped to the theme's foreground or background, whichever contrasts more, when it doesn't (black or white as a last resort). The user's `min_contrast` still applies afterwards.
  - The new theme key `selection_foreground` is read from kitty files, vtt TOML and `[colors]`. The human's quickshell/matugen kitty theme already sets `selection_foreground #5D3F3B` on `selection_background #FFDAD5` (about 7:1), so that is what both views will now show.
  - Tests: `theme::chrome::tests::selected_text_is_readable_without_a_theme_color`, `theme::chrome::tests::theme_selection_foreground_wins_when_readable`, `ui::preview::code::tests::selected_text_reads_on_the_selection`, plus the kitty and TOML parser tests in `theme::formats`.
  - Terminal: the change is the one swap in `paint_terminal` and has no direct unit test (that path isn't unit-tested today). Needs a manual look in a terminal tab.
- Side effect: terminals now also get the "selection color equal to the background" fallback blend that previews had, so an invisible theme selection becomes visible.
- Refactor: `contrast_ratio` / `ensure_contrast` / the WCAG luminance helper moved from `render::grid` to `theme::chrome` (the theme layer sits below render), with their tests.
- Spec note: the spec said "no new config key". `selection_foreground` is a new theme/`[colors]` key, added because the human asked for readability and kitty themes already provide it. It is optional, and without it colors are picked for contrast automatically.
