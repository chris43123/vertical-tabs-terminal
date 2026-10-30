VERDICT: PASS

## Blocking

## Notes
- Round 1 blocker (unbounded favorites section) is fixed: src/ui/sidebar/favorites.rs puts the rows in their own `ScrollArea` with `id_salt(("favorites", expanded))`, `max_height` = 40% of the remaining height (at least one row) and `auto_shrink([false, true])`. The tab list keeps the rest. The drag insertion line is hit-tested and clipped to the visible area.
- Round 1 notes, all answered or fixed: Enter/Esc are left to an open profile popup (`any_popup_open`); a fixed favorites file clears the "invalid favorites file" banner and a config reload re-reports a still-broken one; the missing-path dim now goes through `ensure_contrast` with the user's minimum; "Open in split" on an open favorite is documented in the README; the `~` point was a false alarm (the dialog path goes through `canonical_dir`, which expands it); the nine help rows stay, which is fine.
- Hand-edit warnings ("command dropped", control characters in a name) go to the config banner the vtt way and name the favorite. One small leftover: only the "invalid favorites file" banner is dropped on a successful reload, so a "command dropped" warning stays until dismissed or the next config reload even after the file is fixed. Not blocking.
- Dragging to reorder when the list is scrolled: the area does not auto-scroll while dragging, so a favorite can only be dropped among the rows currently visible. Fine at 40% of the height; the context menu and the CLI `move` cover the rest.
- The hover glyph is now a play triangle when there is no tab and ↻ when there is, which makes the click action clear and matches the "click opens or focuses" behavior. The focused favorite's tint is lighter than the tab's, so the tab stays the main highlight.
- Still no GUI run of the manual criteria (5-10) per build.md; the human should check them, in particular the scrolled list with many favorites in both the expanded and collapsed sidebar.
