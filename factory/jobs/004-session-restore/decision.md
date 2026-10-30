APPROVE

## Why
The latest round (round 3, at 7d6bfc0) has security, design and code all PASS, and no blocking point was ever declined. The one earlier blocking point (dormant tab title contrast) was fixed in d724dd7. The human's feedback in human.md was applied in 7d6bfc0: restored tabs start only their shell in the last known dir and never type the saved command. Acceptance criteria 1-5 and 9 are covered by unit tests, and the human reported the manual checks 6-8 as OK and said to keep `restore_session` on by default, which settles the two items I escalated last time. The diff adds no dependency (Cargo files untouched), no `unsafe`, no keybinding and no platform-specific `cfg` code. The new config key is additive and the config format is otherwise unchanged.

## For the human
- Nothing blocking. Optional glance: a favorite whose recipe has a command now wakes to a bare shell, so its status dot shows a plain shell (intended by your feedback). See `src/app/session.rs` (`wake`).
- Try it: `cargo run` in .worktrees/004-session-restore

## Summary
- Restores tabs, folders, splits, titles, dirs and favorite links at launch from `session.toml`, with dormant tabs that start only their shell on first focus. Saves are debounced and also flushed on exit. `restore_session` (default true) controls it.
- Adds the `vtt session show [--json] | clear | path` CLI, a shared atomic-write helper (`src/fsutil.rs`), `Workspace::restore()`, and README and `config.example.toml` updates.
- Commits: 4cc3da7 Restore open tabs on launch; d724dd7 Keep dormant tab titles above the contrast floor; 7d6bfc0 Start restored tabs with only their shell.
