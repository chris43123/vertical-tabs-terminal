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
