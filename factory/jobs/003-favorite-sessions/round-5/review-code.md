VERDICT: PASS
Reviewed: 337bbca

Checks run in the worktree (CARGO_TARGET_DIR=<repo>/target): `cargo fmt --all -- --check` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test` 157 passed.

## Blocking

(none)

## Notes
- Human #1 (stale favorite warnings): `drop_fixed_problems` drops the "invalid favorites file" line and any "favorite `" line the new list no longer raises. Lines that still hold are kept and not re-added, because `collect_problems` dedupes. Both warning sources share the `FAVORITE_PROBLEM` prefix, and a test covers it.
- Human #2 (auto-scroll): it reuses `ui::preview::autoscroll` and calls it only while a `FavDrag` payload is active with the pointer inside the section. That helper repaints only when the offset changes, so idle cost stays at zero.
- Human #3 (order after a later split): `sync_favorite_order_on_change` runs once per frame after the panes and only re-syncs when `favorite_tabs()` differs from the stored `synced` value. Next/prev, GotoTab and LastTab use `sidebar_order()`, and `Workspace::cycle` takes the order as an argument. `workspace` stays free of rendering. Tests cover `sidebar_order` and `cycle`.
- Other human changes:
  - The resize handle now uses `Sense::click_and_drag()`, so the double-click reset can fire.
  - Open in split into a favorite's view clears `recipe.favorite` through `in_favorite_split`.
  - Both sidebar widths now share `favorite_tile`, with `row_label` and its test.
- The `favorite_tile` and `row_label` change answers the 20:36 request #3 (commit 71f23fb is outside this range, but its follow-ups are in it).
- The new frame-level paths (the handle double-click, the autoscroll hookup, the split-into-favorite branch) have no unit tests because `App` and egui interaction can't be built in tests. These stay in the human's manual pass.
- README, non-blocking: "this favorite's own row stays closed" is slightly off. The code leaves that row as it was, so if it was already open it stays open. Say "stays as it was".
- No `cfg` branches were touched. I found no dead code or debug leftovers. The commit subject style matches the conventions.
