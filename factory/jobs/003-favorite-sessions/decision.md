ESCALATE

## Why
The round 5 reviews (code PASS, design PASS, security PASS) are all clean, nothing blocking was declined, and the automated criteria 1-4 and 11 are met. The diff adds no dependency, no default keybinding (`add_favorite` and `open_favorite_1..9` are unbound) and no `unsafe`. I still can't approve, for two reasons.
- The last commit (337bbca) changed the frame-level behavior and no GUI has run it: auto-scroll while dragging a favorite, the config banner clearing, and next/previous/go-to-tab following a new sidebar order (`Workspace::cycle` now takes that order). Unit tests can't build `App`, so the code reviewer left these to a manual pass. The human's last manual pass covered 71f23fb, not 337bbca. Criterion 9 (drag reorder, context menu, dialog validation, light and dark themes) has no explicit recorded result.
- The branch has things a human should weigh in on: a new `vtt favorites` subcommand and a new on-disk file, changed existing behavior (tab order, Reset following the favorite's current recipe, the `Watcher::changed` and `Workspace::cycle` signatures), and Windows-only path code (`simplify`/`strip_verbatim`) that has only run as string logic on this box. The human accepted these earlier, so this is a confirmation, not a new question.

## For the human
- Run the 337bbca changes by hand: `cargo run` in .worktrees/003-favorite-sessions.
  - Drag a favorite to the top and bottom edge of a scrolled favorites section. It should scroll and drop anywhere (src/ui/sidebar/favorites.rs, `ui::preview::autoscroll`). Idle CPU should stay ~0 afterwards.
  - Put a command containing `\n` into favorites.toml, then fix it. The "command dropped" line should leave the banner (src/app/favorites.rs `drop_fixed_problems`).
  - Split a tab under a favorite after it is already open. Next/previous tab and "go to tab N" must follow the sidebar order (src/app/favorites.rs `sync_favorite_order_on_change`, `sidebar_order`).
  - Also check the earlier round-5 items: double-click on the resize handle resets the height, and "Open in split" of a favorite into another favorite's view opens a plain tab under the target.
- Criterion 9, once: the context menu entries, the dialog validation, the collapsed view and the 20 px tile (dot and badge legibility) in light and dark themes at a large UI zoom.
- Confirm Windows `simplify`/`strip_verbatim` (src/favorites/mod.rs ~365) is accepted unrun on Windows. You said it was.
- Optional README nit (code review): "this favorite's own row stays closed" should read "stays as it was".
- spec.md criterion 2 (duplicate path refused) is obsolete after your instruction to allow the same directory twice.
- Checks: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test` in .worktrees/003-favorite-sessions (157 tests passed per the reviewers).
- If the manual pass is clean, merge with `factory/merge.sh`.

## Summary
- Adds `src/favorites/`: a per-machine TOML favorites file with atomic, re-read-before-write updates, stable `f<N>` ids, single-line command validation on save and load, and a `vtt favorites list|add|set|unset|move|rm|path` CLI. The same directory can be pinned more than once.
- Shows favorites in a "Favorites" section above the tabs. A favorite's row is its tab, with splits grouped under it. Both sidebar widths share one tile design. The section has a resizable height, drag reorder with auto-scroll, a context menu, an add/edit dialog, palette entries and unbound actions. It reloads live through the config watcher. `LaunchRecipe.favorite` makes Reset use the favorite's current recipe. README, config.example.toml and the help window are updated.
- Commits: a043de8 Pin favorite directories above the tabs; ac094cb Check hand-edited favorites on load and cap the pinned list's height; 8abc5cf Make a favorite's row its tab and let one directory be pinned twice; 808dcbc Rename a favorite's tab through its edit dialog; 71f23fb Show favorites as tiles and split into a favorite as a plain tab; 337bbca Clear fixed favorite warnings, auto-scroll favorite drags, keep tab order.
