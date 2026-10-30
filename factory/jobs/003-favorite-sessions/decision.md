ESCALATE

## Why
The round 4 reviews (code PASS, design PASS, security SKIPPED because 8abc5cf and 808dcbc touch no security surface) all pass, and nothing blocking was declined. The diff adds no dependency, no default keybinding (`add_favorite` and `open_favorite_1..9` are unbound) and no `unsafe`. The automated criteria 1-4 and 11 are met, and the README now carries the "dropped bad command" note you asked for. I still can't call every acceptance criterion met, for two reasons.
- The human's macOS pass confirmed only criteria 1, 3, 4, 6 and 7. Criteria 5, 8, 9 and 10 have no recorded result.
- After that pass, the human's feedback (8abc5cf, 808dcbc) reworked the UI substantially: the favorite's row is now its tab, the tab list skips favorite tabs, splits are grouped under the favorite, there is a drag-resize handle, and there is a Favorites header with a hover "+". Unit tests can't build `App`, so the reviewers say none of this has been run in a GUI. That includes the last fix, rename on a favorite's tab. Criterion 2 (duplicate path refused) no longer holds by your own instruction, so spec.md is out of date there.

The branch is also large (24 files, about 3,000 added lines). It changes existing behavior: tab order (favorite tabs are kept at the front of `ws.order`), Reset (it now uses the favorite's current recipe), and the `Watcher::changed` API. It also adds a new `vtt favorites` subcommand and a new on-disk file. Windows-only path handling is exercised only as string logic.

## For the human
- Run the new UI by hand: `cargo run` in .worktrees/003-favorite-sessions. Check the following.
  - Open a favorite. It must not add a row to the normal tab list. Check focus and bell markers, × and middle-click close, and double-click to rename (src/ui/sidebar/favorites.rs).
  - Cmd+R on a favorite's tab, on a split sibling, and with the sidebar collapsed (src/app/tabs.rs `start_rename`, `start_view_rename`, `drop_undrawn_rename`). Nothing should get stuck and the sidebar peek should close.
  - The resize handle: drag it, double-click to reset, and check it persists across restarts (`favorites_height`). Try a long list at a large UI zoom, in light and dark themes.
  - Cmd+Shift+T on a closed favorite tab, then check its row position (src/app/tabs.rs `reopen_closed_tab`).
  - Next/previous tab and "go to tab N" against the sidebar order (`sync_favorite_order`, src/app/favorites.rs).
- Criteria 5, 8 and 10 still have no recorded manual result. Run:
  - `cargo run -- favorites add /tmp --name "🚀 tmp"`
  - `cargo run -- favorites set 1 command htop`, then Reset the tab
  - Activity Monitor for idle CPU
- Decide on the side effect that "Open in split" on a closed favorite splits the focused view and moves that view's tabs under the favorite (build.md, human #2).
- Decide whether to leave the known non-blocking leftovers:
  - A "command dropped" banner stays after the file is fixed.
  - Dragging doesn't auto-scroll a scrolled list.
  - `sync_favorite_order` doesn't run when a tab is later split under a favorite.
  - Ids and unknown profile names can reach stderr unescaped via `diag::warn`.
- Windows `simplify`/`strip_verbatim` (src/favorites/mod.rs ~365) has never run on real Windows. You said this is accepted.
- Checks (152 tests passed per the reviewers): `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test` in .worktrees/003-favorite-sessions.
- If the manual pass is clean, re-run the orchestrator with APPROVE, or merge with `factory/merge.sh`.

## Summary
- Adds `src/favorites/`: a per-machine TOML favorites file with atomic, re-read-before-write updates, stable `f<N>` ids, single-line command validation also enforced on load, and a `vtt favorites list|add|set|unset|move|rm|path` CLI. The same directory can be pinned more than once.
- Shows favorites in a "Favorites" section above the tabs. A favorite's row is its tab, with splits grouped under it. The section has a resizable height, drag reorder, a context menu, an add/edit dialog, palette entries and unbound `add_favorite`/`open_favorite_N` actions. It reloads live through the config watcher, which now reports which file changed. `LaunchRecipe.favorite` makes Reset use the favorite's current recipe. README, config.example.toml and the help window are updated.
- Commits: a043de8 Pin favorite directories above the tabs; ac094cb Check hand-edited favorites on load and cap the pinned list's height; 8abc5cf Make a favorite's row its tab and let one directory be pinned twice; 808dcbc Rename a favorite's tab through its edit dialog.
