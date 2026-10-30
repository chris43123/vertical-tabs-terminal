ESCALATE

## Why
All four round-1 reviews are PASS with no blocking points, and the diff stays inside
the spec. Even so, this branch hits several of the "a human should weigh in" triggers.
It adds new `unsafe` blocks: `killpg`, `getsid`, `getpgid`, `tcgetpgrp` and
`proc_listallpids` on Unix, plus the Job Object FFI on Windows. It adds a direct
dependency entry (`windows-sys` 0.59 in `Cargo.toml`), which is already in the lock
through alacritty. It ships Windows-only code that was only cross-checked with clippy
and never run. It also changes existing behaviour on Windows: closing a tab now ends
grandchildren through `KILL_ON_JOB_CLOSE`. Finally, acceptance criteria 5-9 are
GUI-only checks that build.md marks as not run, and criterion 6 (the process-tree
kill) is only partly covered by a unit test that, per the security review, probably
never reaches the SIGKILL escalation path.

## For the human
- Decide on the new direct dependency `windows-sys` and its feature flags: `Cargo.toml:32-34`.
- Review the `unsafe` process-group kill and the delayed SIGKILL re-scan, including the
  pid-reuse note from the code and security reviews (skip `kill_tree` if the shell has
  already exited): `src/terminal/session.rs:177-318`.
- Review the Windows Job Object code, which compiles but was never run:
  `src/terminal/session.rs:320-380`. Accept or reject the new close-tab behaviour on
  Windows (grandchildren now die when a tab closes). If you have a Windows machine,
  check criterion 6 with `ping -t`.
- Optional, from the UX review: `reset_tab_now` kills the old session before it spawns
  the new one, so a failed respawn closes the tab: `src/app/tabs.rs:126`.
- Run manual criteria 5-9 in the GUI:
  - 5: `cd /tmp`, rename the tab, Reset. The tab keeps its slot and title and the
    prompt is back in the original dir.
  - 6: run `sleep 1000 &` and then `vim`, Reset, confirm. `pgrep sleep` and
    `pgrep vim` from another tab should find nothing.
  - 7: with `confirm_reset = false`, no dialog appears.
  - 8: Duplicate and Reopen closed tab behave as before.
  - 9: idle CPU stays ~0%.
  - Also search "reset" in the palette.
- Run it: `cargo run` in .worktrees/002-launch-recipes-reset

## Summary
- Adds a `LaunchRecipe` (profile, resolved dir, optional command, custom title) to
  every terminal tab. The command is typed into the shell on its first output, and
  closed tabs keep their recipe.
- Adds a "Reset tab" action (unbound, in the palette, the tab menu and help). It kills
  the tab's process tree (Unix: SIGHUP to the session's groups, then a one-shot SIGKILL
  after 500 ms; Windows: Job Object) and respawns the tab in place under the same
  `TabId`. A generation tag drops the old session's late events. A modal asks for
  confirmation when something is running.
- Adds the `confirm_reset` config key (default true) with full `vtt config` support,
  and updates README and `config.example.toml`.

Commits:
- c0a4c38 Add launch recipes and a Reset tab action
