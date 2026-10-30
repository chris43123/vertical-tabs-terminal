VERDICT: PASS
Reviewed: 808dcbc

## Blocking

## Notes
- Round 3 blocking point is fixed. `start_rename` (src/app/tabs.rs) now opens the favorite's edit dialog when `favorite_for_tab` finds a link, and returns before setting `renaming`, so Cmd+R and "Rename tab" in the palette on a favorite's tab no longer leave the sidebar peeked open. `start_view_rename` does the same for a split under a favorite. The dialog is an `egui::Modal` drawn outside the sidebar, so it also works with the sidebar collapsed or in zen mode.
- The `drop_undrawn_rename` guard runs each frame before the sidebar or zen overlay, which covers a rename started on a tab that a favorite claims later. It only assigns `None`, so there is no repaint cost. Split siblings under a favorite are drawn by `tab_row`, so a plain tab rename on a sibling still has an editor and the guard leaves it alone.
- `reopen_closed_tab` now moves a relinked favorite tab out of its folder and re-syncs the order, which matches how a favorite opens normally.
- The unit test covers only the `favorite_for_tab` lookup, because `App` can't be built in tests. That `renaming` stays `None` is left to the manual pass.
- Leftovers from earlier rounds, still not blocking: a "command dropped" banner stays after the file is fixed; no auto-scroll while dragging in a scrolled list; `sync_favorite_order` doesn't run when a tab is later split under a favorite, so next/previous tab can differ from the sidebar order for that sibling.
- Manual pass, by eye: Cmd+R (and "Rename tab" in the palette) on a favorite's tab, on one of its split siblings, and with the sidebar collapsed; Esc and Enter in the dialog return focus to the terminal; reopen a closed favorite tab (Cmd+Shift+T) and check its row position; the Favorites header, the resize handle and a long list at a large UI zoom, in light and dark themes.
