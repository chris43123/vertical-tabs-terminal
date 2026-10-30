APPROVE

## Why
All four round-2 reviews (code, security, UI, UX) are PASS, and no blocking points
were raised or declined. Round 1 was escalated because of the human-weigh-in triggers.
human.md settles each of them. The direct `windows-sys` dependency and the untested
Windows Job Object code, including the new close-tab behaviour, are accepted as is.
The pid-reuse guardrail and spawn-before-kill were requested and are now built:
c1960c6 records (pid, start time), the SIGKILL pass signals only processes that still
match, and `reset_tab_now` starts the new session before it kills the old one. The
human chose the `cmd+shift+r` default on macOS only, and eee9e52 adds it; Linux and
Windows stay unbound. The human passed manual criteria 5-9 on macOS. Criterion 6's
SIGKILL path is now covered by `kill_session_hangs_up_then_kills_what_ignored_it` and
`kill_pass_skips_a_reused_pid`. The only new `unsafe` in round 2 is the macOS
`proc_pidinfo` start-time read. The guardrail the human asked for needs it, and the
security review checked it (`ret == size` before `assume_init`). The diff stays inside
spec.md plus the human's additions. The worktree is clean, and all three commits are
pushed.

## For the human
- Nothing blocking. The open notes are optional follow-ups. The `kill_tree` doc comment
  is one 168-column line that should be re-wrapped (`src/terminal/session.rs:178-180`).
  A process that ignores SIGHUP and forks during the 500 ms grace period can escape the
  SIGKILL pass (security note). The README could also say that a background job at an
  idle prompt is killed without asking (UX note).
- Job 2 has to decide how to validate the recipe `command` when favorites supply it.
- Optional re-check of the round-2 order change (new shell first, then kill): `cd /tmp`,
  rename the tab, then press ⌘⇧R or pick Reset from the tab menu. Run it with `cargo run`
  in .worktrees/002-launch-recipes-reset.

## Summary
- Adds a `LaunchRecipe` (profile, resolved dir, optional command, custom title) to
  every terminal tab. The command is typed into the shell on its first output, and
  closed tabs keep their recipe.
- Adds "Reset tab" to the palette, the tab menu and help: `cmd+shift+r` on macOS,
  unbound elsewhere. It starts a fresh session from the recipe in place under the same
  `TabId`, then kills the old process tree. Unix: SIGHUP to the recorded session
  members, then a one-shot SIGKILL that checks start times. Windows: a Job Object. A
  generation tag drops the old session's late events. A modal asks first when something
  is running.
- Adds the `confirm_reset` config key (default true) with full `vtt config` support,
  and updates README and `config.example.toml`.

Commits:
- c0a4c38 Add launch recipes and a Reset tab action
- c1960c6 Kill only the recorded processes on Reset, after the new shell starts
- eee9e52 Bind Reset tab to Cmd+Shift+R on macOS
