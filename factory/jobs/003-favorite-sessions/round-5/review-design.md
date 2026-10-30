VERDICT: PASS
Reviewed: 337bbca

## Blocking

## Notes
- Round 4 had no blocking points. Reviewed 808dcbc..337bbca (the human-feedback changes: tile, handle double-click, split into a favorite, warning cleanup, auto-scroll, tab order).
- Tile (`favorite_tile` in src/ui/sidebar/favorites.rs): one function draws it in both widths, so they read the same. Colors come from `UiColors` (`raised_sidebar`, `sidebar`, status and badge colors), the corner radius and stroke are the ones the collapsed tile used, and the font scales with the tile (0.6 of its side). The 20 px expanded tile fits the 26 px row and sits clear of the group strip. The badge now stays visible on hover and no longer collides with the hover buttons, which is cleaner than before. `row_label` drops a leading emoji only when something else follows, so a name never becomes empty.
- Handle: `Sense::click_and_drag()` is the right fix for the double-click reset. The resize cursor is kept.
- Auto-scroll: only runs while a `FavDrag` payload is active and the pointer is inside the section, via the existing preview edge-zone helper, which repaints only while the offset changes. No idle repaint loop.
- Tab order: next/previous, go to tab N and last tab now follow the sidebar order, which matches what the user sees. `sync_favorite_order_on_change` compares against the last layout, so it is cheap per frame.
- README and the help window ("Tile dot") match the new behavior in the README's plain style. No new config keys.
- Manual pass, by eye: the tile with a long or emoji-leading name, in light and dark themes and at large UI zoom; the dot and badge corners on a 20 px tile are small, so check they stay legible; double-click on the resize handle; drag a favorite to the edge of a scrolled section; the collapsed strip and zen mode.
