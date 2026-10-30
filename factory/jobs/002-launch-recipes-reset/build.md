# Build log: 002-launch-recipes-reset

## Initial build
Commits: c0a4c38 Add launch recipes and a Reset tab action
Checks: fmt ✓  clippy ✓  test ✓ (120 passed)
Also: clippy `--all-targets -D warnings` clean for `x86_64-pc-windows-msvc` and
`x86_64-unknown-linux-gnu` (cross-checked from macOS).
Pushed: origin/factory/002-launch-recipes-reset at c0a4c38

### What was built
- `src/app/recipe.rs`: `LaunchRecipe { profile, dir, command, custom_title }`, `typed_input()`.
- `Tab.recipe` / `Tab.pending_input`; `ClosedTab` keeps the recipe (dir = last cwd, as before).
- `spawn_tab(recipe, split)`; `new_tab`, `new_tab_in`, `duplicate_tab` (via `new_tab`),
  `reopen_closed_tab`, the settings tab and folder drops all build a recipe.
- Command typed on the session's first `Wakeup` (first output), not on a timer.
- `Listener` carries a `Source { tab, generation }`; `process_events` drops events whose
  generation isn't the tab's current session's.
- `Session::kill_tree()`: Unix signals every process group in the shell's session
  (found via `getsid` over all pids: `/proc` on Linux, `proc_listallpids` on macOS) plus the
  PTY foreground group, SIGHUP + SIGCONT, then a one-shot thread re-scans after 500 ms and
  SIGKILLs what's left. Windows: Job Object with `KILL_ON_JOB_CLOSE`, terminated on reset
  (`windows-sys` 0.59, already in the lock via alacritty; only feature flags added).
- `App::reset_tab` / `reset_tab_now` (in place, same `TabId`, clears osc/auto title,
  process, activity, bell); new `src/ui/reset_prompt.rs` modal (egui `Modal`, `UiColors`).
- `reset_tab` action (unbound everywhere), palette, help "Tabs" section, tab context menu
  "Reset" (terminal tabs only), `confirm_reset` config key, README + `config.example.toml`.

### Acceptance criteria
1. ✓ Recipe resolves dir (requested, else profile cwd, else home), keeps profile/command/title —
   `app::recipe::tests::{dir_prefers_requested_then_profile_then_home, new_keeps_profile_and_resolves_dir, keeps_command_and_title_when_cloned}`
2. ✓ Command gives exactly `command + "\r"`, none without — `typed_input_is_command_and_carriage_return`
3. ✓ `vtt config set/get/unset/check/reference` for `confirm_reset` — reference tests pass, and
   manual run with `HOME` pointed at a scratch dir: set false → get `false`, check ok,
   `set maybe` rejected (exit 1), unset → `true`, reference lists it.
4. ✓ `reset_tab` parses, no default chord on any platform — `keybinds::tests::reset_tab_is_an_unbound_action`;
   it's in `ACTIONS`, which the palette lists (`every_action_is_listed` for help). Palette search
   "reset" not checked by hand in the GUI.
5. ✗ (not run) Reset idle tab keeps slot/split/folder/title, fresh shell in recipe dir — needs a
   GUI manual check; code keeps the same `TabId` and only swaps `tab.content`.
6. ~ Reset with `sleep 1000 &` + `vim`: confirm modal path implemented; process-tree kill covered
   by `terminal::session::tests::kill_session_ends_background_jobs_and_ignorers` (a session with a
   background job and a SIGHUP-ignoring child; both gone after kill). GUI and Windows `ping -t`
   check not run.
7. ✗ (not run) `confirm_reset = false` skips the dialog — manual; code path is a single `if`.
8. ✗ (not run) Duplicate / Reopen closed tab unchanged — manual; reopen still uses the last cwd
   and custom title, duplicate still goes through `new_tab`.
9. ✓ (by construction) No new repaint or poll loops: the command is written on a PTY event, the
   modal draws only while open, SIGKILL escalation is one sleeping thread per reset. Not measured.
10. ✓ fmt, clippy, test pass; README (Features, keybinding row, actions list, config copy) and
    `config.example.toml` updated.

### Notes for review
- README's copy of the config was already behind `config.example.toml` (it lacks
  `zen_edge_width`, `focus_follows_mouse`, `hide_pointer_when_typing`, `copy_on_select`). I only
  added the new key there; syncing the rest is a separate docs fix.
- On Windows the job object also ends grandchildren when a tab is closed (handle close kills the job).
- If respawning fails during a reset, the old (already signalled) session stays and its exit
  closes the tab as before; the failure is reported in the problems banner.

## Human feedback (human.md, 2026-10-30T15:06:39Z)
Commits: c1960c6 Kill only the recorded processes on Reset, after the new shell starts,
eee9e52 Bind Reset tab to Cmd+Shift+R on macOS
Checks: fmt ✓  clippy ✓  test ✓ (121 passed); clippy also clean for
`x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`; kill tests run 6x, no flakes.
Pushed: origin/factory/002-launch-recipes-reset at eee9e52

### Acceptance criteria
1-4, 10. ✓ as before (4 now: `keybinds::tests::reset_tab_is_bound_on_macos_only`).
5-9. ✓ manual, passed on macOS by the human on the round-1 build (reset in place,
   `sleep &` + vim killed, `confirm_reset = false`, duplicate/reopen, idle CPU, palette).
6. Also ✓ `terminal::session::tests::kill_session_hangs_up_then_kills_what_ignored_it` now
   reaches the SIGKILL path: it waits until the HUP-ignoring child is running (marker file
   written after its trap), checks it alone survives the hangup, then that the delayed pass
   ends it. Windows `ping -t` not run (no Windows machine; accepted by the human).

### Review responses
- human #1 (windows-sys direct dependency): accepted as is, no change.
- human #2 (pid reuse in the SIGKILL pass): fixed in c1960c6. The first pass records each
  session member as (pid, start time) (Linux `/proc/<pid>/stat` field 22, macOS
  `proc_bsdinfo.pbi_start_tv*`) and SIGHUPs it. The SIGKILL pass signals only those still
  alive with the same start time; nothing is looked up by the old shell's pid later. The
  shell's start time is recorded at spawn, so a shell that already exited (pid maybe reused)
  is never signalled (this also covers the code/security review note). New test
  `kill_pass_skips_a_reused_pid`.
- human #3 (Windows Job Object, close-tab behaviour): accepted as is, no change.
- human #4 (spawn before kill): fixed in c1960c6. `reset_tab_now` starts the new session
  first; on failure it returns with the error in the problems banner and the old session
  untouched. Only then is the old tree killed and the content swapped.
- human: `reset_tab` default `cmd+shift+r` on macOS only: done in eee9e52 (keybinds, README
  table/Features/config copy, `config.example.toml`; help shows it from the keymap).
- code review note (reopen sets custom_title twice): fixed in c1960c6, only previews take
  `closed.custom_title` now.
