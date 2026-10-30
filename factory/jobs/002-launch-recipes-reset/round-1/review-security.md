VERDICT: PASS

## Blocking

## Notes
- src/terminal/session.rs (kill::session, delayed SIGKILL): the escalation thread
  re-scans session `sid` 500 ms later. If every member is already gone and reaped, `sid` is
  free, and a new process that got the same pid and called `setsid` would be SIGKILLed.
  macOS and Linux hand out pids in sequence, so this only matters near pid wraparound;
  acceptable. A cheap guard would be to skip the SIGKILL pass when the first scan
  showed the shell exited, or to SIGKILL only pgids that were in the first SIGHUP set
  and are still in session `sid`.
- Every group it signals is either a pgid of a process whose `getsid == sid`, or
  `tcgetpgrp` on our own PTY. That can only reach groups in the tab's own session, and
  `sid <= 1` is rejected, so vtt's own group and init are never hit. The thread uses only `sid`, not
  the PTY fd, so a closed or reused fd is never read after the session drops. The unsafe
  blocks (killpg/getsid/getpgid/tcgetpgrp/proc_listallpids) are sound. The second
  `proc_listallpids` result is clamped with `truncate`, so a short or failed return can't
  read uninitialised memory.
- src/terminal/session.rs `kill_session_ends_background_jobs_and_ignorers`: it runs in
  about 0.1 s, under the 500 ms grace. It calls `kill::session` as soon as the `sh` is
  visible, which is probably before the subshell has run `trap '' HUP`. So the
  SIGKILL-for-ignorers path is probably not exercised. Waiting until two or more pids are in the
  session (or sleeping ~200 ms first) would make the test cover escalation. Not a
  security blocker: the code path itself is correct.
- Windows `Job::for_process`: the handle is wrapped in `Job` before the fallible calls,
  so it's closed on every failure path. A process that is already in a job that doesn't
  allow nesting or breakaway makes `AssignProcessToJobObject` fail; that leads to `None`, with no kill
  on reset (graceful). `windows-sys 0.59.0` is already in Cargo.lock; there's no new crate or build
  script.
- src/app/events.rs: dropping every event whose generation isn't current also drops a
  replaced session's late `PtyWrite`, `ClipboardStore`/`ClipboardLoad` and title events.
  That's good: a killed program can't reach the clipboard or the new shell. The
  `unwrap()` on `tab.session()` after the generation check can't panic.
- Recipe command (src/app/recipe.rs `typed_input`): only trailing CR/LF are trimmed. An
  interior newline or control byte is typed as is and runs as several commands. In
  job 1 nothing sets a command. For job 2, when the favorites file supplies one, decide
  whether to reject control characters (other than tab) at parse time, and treat the
  file as trusted user config, as the spec says.
- The reset modal shows `tab.title()`, which can be a program-set OSC title. egui renders
  it as plain text, so a misleading title can only change the label, not the
  buttons. No action needed.
