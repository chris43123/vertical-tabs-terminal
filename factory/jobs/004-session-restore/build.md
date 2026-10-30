# Build log: 004-session-restore

## Initial build
Commits: 4cc3da7 Restore open tabs on launch
Checks: fmt ✓  clippy ✓  test ✓ (176 passed)
Pushed: origin/factory/004-session-restore at 4cc3da7

### What was built
- `src/session/mod.rs`: `Session` data (`[[tab]]`, `[[folder]]`, `[[view]]` with an untagged
  `layout` tree, `version`, `active`), `snapshot()` / `Session::rebuild()`, `Store`
  (load, `load_for_restore` with `.bad` quarantine, atomic save, clear).
- `src/session/cli.rs`: `vtt session show [--json] | clear | path`, dispatched from `main`.
- `src/fsutil.rs`: `write_atomic` pulled out of the favorites `Store`, used by both.
- `Workspace::restore()` constructor (validates/repairs saved views, folders, active view).
- `src/app/session.rs`: restore at launch, `Content::Dormant`, `wake_visible()` (called before
  panes and at end of frame, so any path that shows a tab wakes it), debounced save,
  `on_exit`/`save` flush, `favorite_status()` (dormant link = Idle dot).
- `LaunchRecipe::ephemeral` keeps the settings-editor tab out of the session.
- `restore_session` in Config, reference, config.example.toml, README copy; README Features,
  "Session restore" section and its "From the command line" paragraph.

### Deviation from the spec
- Dirty tracking: instead of setting a flag at every mutation site, each frame compares the
  current snapshot with the last written one (`session_tick`). It catches every path (incl.
  ones added later), only runs on frames that happen anyway, and schedules a single
  `request_repaint_after` 2 s from the first difference; nothing runs while idle. Debounce is
  measured from the first change, so a continuous change (ratio drag) still saves every 2 s.
- A dormant tab whose shell fails to start is closed (with the banner warning) rather than
  left dormant, so it isn't retried every frame.
- Open question in the spec (re-type commands on restore): yes, matching Reset, except when the
  profile had to be substituted.

### Acceptance criteria
1. ✓ Round trip of order, folders, splits+ratios, view names, focus, active view, titles,
   commands, favorite links — `session::tests::snapshot_then_rebuild_round_trips_the_layout`,
   `ratios_and_split_direction_survive_the_file`, `workspace::tests::restore_rebuilds_folders_splits_and_focus`,
   `restore_repairs_inconsistent_views`, `session::tests::save_and_clear`.
2. ✓ Saved dir is last known cwd — `session::tests::saved_dir_is_the_last_known_cwd`.
3. ✓ Fallbacks — `unknown_profile_uses_the_default_without_its_command`,
   `missing_dir_falls_back_to_the_profile_then_home`, `removed_favorite_drops_the_link`,
   `corrupt_or_newer_file_is_moved_aside`, `hand_edited_layouts_never_panic`.
4. ✓ CLI — `session::cli::tests::*` (show, show --json, empty, clear twice, path, corrupt
   file exit 1, usage exit 2). Also ran `vtt session path|show|bogus` by hand (exit 0/0/2).
5. ◐ `restore_session` known to config tooling — `every_config_field_is_documented`,
   `every_option_is_in_the_example`, `parses_values_by_type_and_range`,
   `reference_lists_defaults_and_actions`; `vtt config get restore_session` prints `true`.
   The "false writes nothing" half is enforced in `restore_session()`/`session_tick()`/
   `save_session()` but the GUI manual check was not run.
6. ✗ Manual GUI check (5 tabs, folders, split, relaunch, `ps`) — not run; needs a human.
7. ✗ Manual `kill -9` check — not run; needs a human.
8. ✗ Manual idle CPU check — not run; needs a human (design: no timer while snapshot unchanged).
9. ✓ fmt/clippy/test pass; README and config.example.toml updated.

## Round 1
Commits: d724dd7 Keep dormant tab titles above the contrast floor
Checks: fmt ✓  clippy ✓  test ✓ (176 passed)
Pushed: origin/factory/004-session-restore at d724dd7

### Acceptance criteria
Unchanged from the initial build (1-4, 9 ✓; 5 half-manual; 6-8 need a human). This round only changed a color.

### Review responses
- design #1: fixed in d724dd7. Dormant title is now `ensure_contrast(mix(text, sidebar, 0.55), sidebar, DIM_CONTRAST.max(min_contrast))`, and `DIM_CONTRAST` moved to `ui/sidebar/mod.rs`, shared with favorites.
- code, security: PASS. Optional notes (early return in `on_exit`, 0600 file mode, `printable()` on parse errors) not taken.

## Human feedback (2026-10-30T21:30:59Z)
Commits: 7d6bfc0 Start restored tabs with only their shell
Checks: fmt ✓  clippy ✓  test ✓ (176 passed)
Pushed: origin/factory/004-session-restore at 7d6bfc0

### Acceptance criteria
1-5, 9. ✓ Unchanged (same tests as before).
6-8. ✓ Manual checks reported OK by the human. `restore_session` stays on by default.

### Review responses
- human (no re-typed command): fixed in 7d6bfc0. `App::wake` no longer sets
  `pending_input` from the recipe, so every restored tab (visible at launch or woken on
  first focus, any profile) starts only its shell in the last known dir. The command is
  kept in the recipe and `session.toml` (round trip unchanged), so an explicit Reset on a
  running tab still types it; Reset on a dormant tab just wakes it, without the command.
  README "Session restore" bullet updated; `config.example.toml` already said running
  programs aren't restored. No unit test: `wake` needs a live App/PTY; the change is a
  single assignment, covered by the manual flow.
