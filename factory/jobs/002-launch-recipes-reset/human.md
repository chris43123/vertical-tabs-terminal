## 2026-10-30T15:06:39Z

Decisions on decision.md "For the human":
1. windows-sys direct dependency: accepted as is (no Windows machine to test on).
2. Pid reuse in the delayed SIGKILL pass: add a guardrail. Don't re-scan the session by the old shell's pid after the grace period. Record the exact processes killed in the first pass (pid plus start time) and, at the SIGKILL pass, kill only those still alive with the same start time. Strengthen the kill test so it actually reaches the SIGKILL path (child ignoring SIGHUP is running before reset).
3. Windows Job Object code and the new close-tab behaviour: accepted as is, untested.
4. Fix it: reset_tab_now must spawn the new session first and only kill/replace the old one once the spawn succeeded, so a failed respawn leaves the tab (and its old session) in place with the error in the problems banner.

Also: bind reset_tab by default to cmd+shift+r on macOS only (Linux/Windows stay unbound). The human chose this. Update the keybinding table, help, config.example.toml and README examples.

Manual criteria 5-9 passed on macOS on the round-1 build (reset in place, sleep & + vim killed, confirm_reset=false, duplicate/reopen, idle CPU, palette search).
