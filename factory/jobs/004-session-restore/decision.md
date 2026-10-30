ESCALATE

## Why
All three round-2 reviews are PASS or SKIPPED (security passed in round 1; design's one blocking point, the dormant title contrast, was fixed in d724dd7) and no blocking point was declined. I am escalating for two reasons. First, acceptance criteria 6, 7 and 8 (relaunch with 5 tabs and `ps`, `kill -9` after a change, idle CPU) and the GUI half of 5 (`restore_session = false` writes nothing) are manual checks nobody has run; the box has no GUI. Second, the default `restore_session = true` changes what every launch does (it used to open one fresh tab), and it stores recipe commands in `session.toml` that get re-typed into shells when a tab is first focused. The spec left that as an open question. A human should accept both. The branch adds no dependency, `unsafe`, keybinding or platform-specific code.

## For the human
- Decide whether restoring tabs by default and re-typing saved commands at first focus is right (build.md "Deviation"; `src/app/session.rs`, `src/session/mod.rs`). A substituted profile never gets a command.
- Dirty tracking compares a full snapshot on every frame that runs (`session_tick` in `src/app/session.rs`) instead of setting flags at each mutation site. Review and code both called it fine at this scale, but it clones the workspace during heavy output. Confirm you accept that.
- Manual pass (criteria 6-8, plus 5):
  - Open 5 tabs in two folders with one split, `cd` in each, quit, relaunch. Layout and directories should come back, `ps` should show shells only for the visible view, and clicking a dormant row should start its shell in its saved dir.
  - `kill -9` a few seconds after a structural change, then relaunch. The change should be there. Repeat with zen mode on: a `cd` made there is only saved on a normal quit.
  - Idle CPU should stay about 0% with a restored session.
  - With `restore_session = false`, the file should not change.
  - Check by eye that a dormant row is clearly dimmer than an active one on a light theme.
  - Check that every wake route works (click, switcher, next/prev, favorite click, drop into a split).
- Command: `cargo run` in .worktrees/004-session-restore. Then run `cargo run -- session show`, and `cargo run -- session path`.

## Summary
- Saves the session (tabs, folders, splits with ratios, titles, last cwd, commands, favorite links) to `session.toml` about 2 s after a change and on quit, and restores it at launch. Only visible panes start shells; the rest are dormant and dimmed until first shown. Bad or newer files are moved to `session.toml.bad`.
- Adds `vtt session show [--json] | clear | path`, the `restore_session` config key (reference, example, README), and moves the atomic write into `src/fsutil.rs`, shared with favorites.
- Commits: 4cc3da7 Restore open tabs on launch; d724dd7 Keep dormant tab titles above the contrast floor. fmt, clippy and tests (176) are clean.
