VERDICT: CHANGES

Checks run in the worktree: `cargo fmt --all -- --check` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test` 145 passed.

## Blocking
1. src/favorites/mod.rs:289 (`canonical_dir`, also `same_dir` at :302) — favorite paths are stored with `std::fs::canonicalize`. On Windows that returns a verbatim path (`\\?\C:\Users\me\api`). That string would then be written to favorites.toml, shown in the tooltip, `list`, `--json` and the dialog, and passed as the shell's start directory. cmd.exe refuses `\\?\` as a current directory, so Reset/open of a favorite would not land in the directory, and the user sees an ugly path. Nothing else in the repo canonicalizes, so there is no existing helper. vtt supports Windows (conventions: all three platforms), so this has to work there. Do instead: strip the `\\?\` prefix under `cfg(windows)` (`\\?\UNC\` becomes `\\`) after canonicalizing, in one small helper used by both `canonical_dir` and `same_dir`. Add a unit test of the stripping function that runs on every OS, since it is a pure string function.

## Notes
- src/favorites/mod.rs:`validate` — `Favorites::set` re-validates the whole draft, so `set <fav> name X` or `set <fav> command htop` fails with "<path> doesn't exist" when the favorite's directory is temporarily missing (unmounted drive, removed checkout). The same happens to the dialog's Edit. Acceptance 7 only needs add to require an existing path. Consider checking the path only when it is added or changed, so a missing-directory favorite can still be renamed or have its command fixed.
- src/favorites/mod.rs:`to_toml` header says "see `vtt favorites --help`", but `--help` falls through to `usage()` and exits 2 (usage on stderr). Either accept `--help`/`-h` with exit 0 and the usage on stdout, or reword the comment to say `vtt favorites`.
- src/app/favorites.rs:`open_favorite` — "Open in split" in the context menu only splits when the favorite has no live tab. If it is already open it just focuses it. That matches "focus if open" but the menu label may surprise; fine to leave, or say so in the help text.
- src/ui/sidebar/mod.rs:1 — the first doc line was extended without re-wrapping and is now over 120 columns. Re-wrap it.
- src/ui/favorite_dialog.rs — Esc closes the whole dialog even when the profile ComboBox popup is open. Minor.
- Manual criteria 5 to 10 were not run in a GUI (build.md says so). The code paths I read for them (link by id, existence check on click, `current_recipe` for Reset, watcher reporting which file changed, no new timers or repaints) match the spec.
