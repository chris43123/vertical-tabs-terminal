VERDICT: PASS

## Blocking

## Notes
- Round-1 note (pid reuse in the delayed SIGKILL pass): fixed in c1960c6 as the human
  asked. `kill::session` snapshots `Proc { pid, start }` for each session member once, and
  both passes go through `Proc::signal`, which checks the start time again right before
  `kill`. The shell's own start time is recorded at spawn, so a shell that exited and had
  its pid reused is skipped up front. The re-scan by `sid` is gone. The only gap left is the
  usual check-then-kill race of microseconds, which is acceptable.
- The other side of the snapshot: a process that ignores SIGHUP and forks *after* the
  first scan (during the 500 ms grace) is not in the list, so it survives the SIGKILL pass.
  Dropping the session hangs up the PTY, so the kernel still sends SIGHUP to the
  foreground group, but a background ignorer that forks late can escape. That's a narrow
  case and the human chose this trade-off. Not a blocker. If it matters later, a second
  `members(sid)` scan at SIGKILL time, filtered to pids whose start time is *earlier* than
  the reset moment, would catch them without reopening the reuse issue.
- `start_time`: the Linux parser uses `rfind(')')` and `split_whitespace().nth(19)`, and
  every step is `?`, so a hostile process name (spaces or parentheses in `comm`) can't
  shift fields or panic. The macOS `proc_pidinfo` call checks `ret == size` before
  `assume_init`, so the unsafe block is sound. `pbi_start_tvsec` and `pbi_start_tvusec` are
  `u64` in libc and `* 1_000_000` can't realistically overflow. For another user's pid,
  `start_time` returns `None` and the process is never signalled, which is the right
  failure mode.
- The empty-process-table fallback (`killpg` on `tcgetpgrp` and the shell's group) only
  runs on platforms with no `all_pids`, after the shell's start time has been checked, and
  it rejects groups `<= 1`. The `sid <= 1` guard is now `shell.pid > 1` and
  `Proc::signal`'s `pid > 1`, which is equivalent.
- `reset_tab_now` spawns first: when `start_session` fails, it returns before
  `kill_tree`, so nothing is signalled and the old session and its generation stay
  current. When it succeeds, the old tree is killed and then dropped. The stale-generation
  filter in `src/app/events.rs` still drops late `PtyWrite` and clipboard events from the
  old program.
- Tests: `kill_session_hangs_up_then_kills_what_ignored_it` now waits for the HUP-ignoring
  child (marker file), asserts it survives the hangup, and then asserts the SIGKILL pass
  ends it, so escalation is covered. `kill_pass_skips_a_reused_pid` covers the
  start-time guard. Both pass locally on macOS (0.54 s).
- eee9e52 (macOS `cmd+shift+r` default) touches nothing in this lane.
- Carried over for job 2: recipe `command` is typed as is, including interior control
  bytes. Decide on validation when favorites supply it.
