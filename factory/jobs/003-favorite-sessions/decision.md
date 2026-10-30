ESCALATE

## Why
All four round-2 reviews are PASS. Every round-1 blocker (Windows `\\?\` paths,
control characters only checked on write, the unbounded favorites section) was fixed
in ac094cb, and the one declined note (`~` in the dialog) was declined for a good
reason. The diff adds no dependency, no default keybinding (`add_favorite` and
`open_favorite_1..9` are unbound on every OS, and a test checks this) and no `unsafe`.
The automated criteria 1-4 and 11 are met. Six of the eleven acceptance criteria
(5-10) are manual GUI checks, and build.md and every reviewer say nobody has run them.
They cover the main feature: clicking to open or focus, live reload from the CLI with
emoji, the missing-path toast, Reset picking up a changed command, drag reorder, the
context menu, the collapsed view, dialog validation, light and dark colors, and idle
CPU. So I can't call every criterion met. The change is also large (about 2,600 added
lines in 24 files) and changes some existing behavior that a human should accept:
- Reset now restarts from the favorite's current recipe and updates the tab's profile.
- `vtt` gets a new `favorites` subcommand.
- A new per-machine file stores state, and its location differs on each OS (Windows and
  macOS paths can't be exercised on this box).

## For the human
- Run manual criteria 5-10 from spec.md:89-93: `cargo run` in
  .worktrees/003-favorite-sessions. In a second terminal, run
  `cargo run -- favorites add /tmp --name "🚀 tmp"`, then `... favorites list --json`,
  `... favorites set 1 command htop`, and `... favorites rm 1`.
- Click and focus behavior, and the missing-path toast:
  src/app/favorites.rs:242 (`open_favorite`), :272 (`open_favorite_at`).
- Changed Reset behavior: src/app/tabs.rs:127 (`reset_tab_now` now calls
  `current_recipe`, src/app/favorites.rs:208, and sets `tab.profile` and `tab.recipe`
  from it). Confirm you want Reset to follow later edits to the favorite.
- Sidebar section height cap (40% of the sidebar, own scroll area):
  src/ui/sidebar/favorites.rs:44-46. Check it with many favorites, both expanded and
  collapsed, and with a large UI zoom.
- New on-disk file: `dirs::data_dir()/vtt/favorites.toml` (src/favorites/mod.rs:422).
  On macOS it sits next to config.toml. Accept the format and location.
- Windows-only path handling (`simplify`/`strip_verbatim`, src/favorites/mod.rs:365-373)
  is only tested as string logic. Nothing has exercised it on real Windows.
- A hand-edited bad command is dropped from the file the next time it is written
  (src/favorites/mod.rs:315 `sanitize`). It warns but isn't in the README; decide
  whether it should be.
- Small reviewer leftovers, not blocking: ids and unknown profile names can reach stderr
  unescaped through `diag::warn` (security). A "command dropped" banner stays after the
  file is fixed, and dragging doesn't auto-scroll a scrolled list (ux).
- Checks: `CARGO_TARGET_DIR=<repo>/target cargo fmt --all -- --check && cargo clippy
  --all-targets -- -D warnings && cargo test` in .worktrees/003-favorite-sessions
  (151 tests passed per the reviews).

## Summary
- Adds `src/favorites/`: a TOML favorites file per machine, with atomic writes that
  re-read the file before each change, stable `f<N>` ids, a single-line command rule
  that is also enforced on load, and a `vtt favorites list|add|set|unset|move|rm|path`
  CLI.
- Shows favorites as pinned rows above the tabs: status dot, open-or-focus on click,
  reset on hover, context menu, drag reorder, an add/edit dialog, palette entries and
  unbound `add_favorite`/`open_favorite_N` actions. The rows reload live through the
  existing config watcher, which now reports which files changed.
- Links tabs to favorites (`LaunchRecipe.favorite`) so Reset uses the favorite's
  current recipe. Updates the README, config.example.toml and the help window.

Commits: a043de8 Pin favorite directories above the tabs; ac094cb Check hand-edited
favorites on load and cap the pinned list's height.
