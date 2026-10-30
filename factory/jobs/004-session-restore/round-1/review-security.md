VERDICT: PASS
Reviewed: 4cc3da7

## Blocking

## Notes
- Checked in my lane, no problems: no `unsafe`/FFI and no new dependency. `Session::parse` returns `Err` for bad TOML and for a newer `version`. `Workspace::restore` drops out-of-range, duplicate and foreign leaves and bad folder indices. `Layout::to_node` clamps NaN and out-of-range ratios. Index arithmetic uses `u64` `TabId` with TOML `i64` inputs, so it can't overflow. I fed `vtt session show` layouts nested 100, 200 and 2000 deep. Each gave exit 1 with an error message, with no crash or stack overflow.
- Output safety: `vtt session show` escapes control characters through `printable()`, the same as the favorites CLI. Titles, commands and a hostile directory name therefore can't send escape sequences to the user's terminal. `--json` is escaped by serde_json. Nothing a program prints in the PTY can reach the session code, so no new OSC surface.
- Command execution: nothing runs until the tab is shown. A substituted (unknown) profile drops the saved command. Saved commands come only from recipes (profile or favorite), not shell history. The file is in the user's own data dir, so editing it needs the same rights as editing the shell rc.
- `write_atomic` writes `.session.toml.<pid>.tmp` and renames it, and removes the temp file on failure. It is shared with favorites, so the behavior is unchanged. The file is created with the default umask (typically 0644) and may hold favorite commands. If you want to harden it, create it with 0600 on Unix. Favorites has the same property, so this is not required here.
- `load_for_restore` moves an unreadable or unparseable file to `.bad`, which overwrites an older `.bad`. That is acceptable. A permission error on read also quarantines the file. If the rename fails the warning still shows, and the next save would replace the file. Low risk.
- Hand-edited huge `[[view]]` trees make `Workspace::restore` quadratic in leaf count (`leaves.iter().filter`, `used.contains`). That is a CPU cost only, on the user's own file, with no crash.
- A TOML parse error message may quote the file's offending line when `vtt session show` prints it. TOML rejects raw control characters in strings, so this is not a practical injection route. If you want to be thorough, run the error text through `printable()` as well.
- Not verified, because there is no GUI here: the `restore_session = false` no-write path (acceptance 5, manual) and idle CPU (8). The code gates `save_session` and `session_tick` on `config.restore_session`, including in `on_exit` and `save`.
