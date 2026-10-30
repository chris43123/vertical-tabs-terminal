# Copy anything out of a preview, however long

## Goal
Copying a long file or a long passage out of a preview pane should take one gesture, not
a careful drag. Today each code row is its own egui `Label` inside a virtualized
`show_rows` area. Selection lives on those widgets, so it breaks once rows scroll out of
view, and dragging past the pane edge doesn't scroll. The fix keeps the selection in
document coordinates instead of on widgets, lets you select by line number, adds one-click
"copy all", and adds edge auto-scroll as the expected baseline.

## Scope
- In:
  - **Copy button** in the preview toolbar (text, code and markdown, rendered or source).
    With a selection it copies the selection; without one it copies the whole file's text.
    For rendered markdown with no selection it copies the markdown source.
  - **Document selection for code/source views** (`CodeView::show` and `show_wrapped`):
    anchor/head as (line, byte offset), so it survives virtualization, wrap toggling and
    any scroll distance. Drag to select, Shift+click to extend, double-click a word,
    triple-click a line.
  - **Line-number selection**: click a gutter number to select that line; Shift+click
    another number to select the whole range; drag in the gutter to select whole lines.
    This is the "nicer solution": select lines 1-2000 with two clicks and no dragging.
  - **Edge auto-scroll** while a selection drag is active and the pointer is near or past
    the body's top/bottom edge, for code views and for rendered markdown. Speed grows
    with the distance past the edge. In code views with wrap off, it scrolls sideways too.
  - Keys while a preview pane is focused: `Ctrl+A` / `⌘A` selects all,
    `Ctrl+C`, `Ctrl+Shift+C` / `⌘C` copy, `Esc` clears. These are the standard editor and
    browser keys for these exact actions, scoped to the focused preview. Their effect in a
    terminal does not change.
  - Right-click menu on the text body: Copy, Copy all, and **Copy reference**
    (`path:12-40`, the format agents and editors understand).
- Out (non-goals): editing, find-in-preview, a new config key, a new `keybinds::Action`,
  a custom selection model for *rendered* markdown (it keeps egui's own label selection
  and gains only auto-scroll and the Copy button), copying with syntax colors or rich text.

## Design
- `src/preview/selection.rs` (new, pure, no egui): `Selection { anchor, head }` over
  `Text` positions. Includes ordering, `text(&Text) -> String` (lines joined with `\n`),
  word and line expansion, `select_lines(a, b)`, `all(&Text)`, and
  `reference(path, &Text) -> String`. Also a pure `autoscroll_step(overshoot_px, dt) -> f32`
  with a capped speed. `Preview` holds `selection: Option<Selection>` and clears it in
  `reload_if_changed`.
- `src/ui/preview/code.rs`: one `Sense::click_and_drag` response over the body instead of
  selectable labels. Labels become `.selectable(false)`. Hit-test pointer → row → (line, byte)
  using the visible row's galley (`cursor_from_pos`), which handles wide chars. Wrapped rows
  map through `Wrapped::rows`. Paint selection rects behind visible rows only.
- `src/ui/preview/mod.rs`: Copy button, context menu, and auto-scroll (drag active plus
  pointer within ~24 px of the edge or past it → `scroll_with_delta` and `request_repaint`).
  Markdown auto-scroll lives in `ui/preview/markdown.rs`'s ScrollArea.
- Copy routing: `App::handle_keyboard` → `copy_selection` falls back to the focused
  preview's selection when there is no terminal session. All clipboard writes go through
  `App::set_clipboard`.
- Selection color: add `selection` to `theme::chrome::UiColors`, from `Theme.selection`
  (no hardcoded `Color32`; check it against minimum contrast).
- Docs: README "Files and previews" (copying, line-number selection, Copy reference) and
  the keybinding table row for copy/select all in previews; `ui/help.rs` mouse/keys list.

## Acceptance criteria
1. `Selection::text` returns exact text for single-line, multi-line, reversed
   (head before anchor) and multi-byte ranges — verify: unit test in `preview::selection`.
2. Word, line and line-range expansion, select-all and `reference` (`a.md:3`, `a.md:3-9`)
   are correct — verify: unit test in `preview::selection`.
3. `autoscroll_step` is 0 inside the body, increases with overshoot and is capped — verify:
   unit test in `preview::selection`.
4. A selection survives a file reload only as `None` (cleared) — verify: unit test in `preview`.
5. In a 3000-line file, clicking gutter line 1 then Shift+clicking line 3000 and pressing
   Ctrl+C puts all 3000 lines on the clipboard — manual.
6. Dragging from mid-file to below the pane scrolls steadily and extends the selection.
   Releasing stops scrolling. Works with wrap on and off, and in rendered markdown — manual.
7. Toolbar Copy with no selection copies the whole file (markdown source when rendered) — manual.
8. Ctrl+C in a focused *terminal* behaves exactly as before (copy or SIGINT) — manual.
9. Idle CPU stays ~0% with a preview open and a selection active. Repaints happen only
   while dragging — manual (top/Activity Monitor).
10. `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and
    `cargo test` pass. README and the help window are updated.

## Risks
- **Keys:** `Ctrl+A`/`Ctrl+C` are conventional for exactly these actions and only act when
  a preview has focus, so they fit "no invented keybinds". No Alt chord, no new global
  binding. If reviewers disagree, ship Copy via the button and menu only.
- **egui interplay:** switching code rows to non-selectable labels removes egui's native
  selection there. Rendered markdown keeps it, so egui's `Event::Copy` handling and ours
  must not both fire. Check that `egui_wants_keyboard_input` doesn't swallow Ctrl+A.
- **Performance:** selecting all of an 8 MB file builds the string only when copying, not per
  frame. Painting covers visible rows only.
- **Platform:** Wayland primary selection (middle-click paste) is not part of this job.
  `arboard` handles large clipboard writes on all three platforms, but test Wayland.
- Open question: should "Copy reference" also go in the palette as an unbound
  `copy_preview_reference` action? Left out for now to stay slim.
