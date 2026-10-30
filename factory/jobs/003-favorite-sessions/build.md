## Initial build
Commits: a043de8 Pin favorite directories above the tabs
Checks: fmt ✓  clippy ✓  test ✓ (145 passed)
Pushed: origin/factory/003-favorite-sessions at a043de8

### Acceptance criteria
1. ✓ Resolve `<fav>` by position, id, name; ambiguous name errors with ids — `favorites::tests::resolves_by_position_then_id_then_name`, `a_position_wins_over_a_numeric_name`
2. ✓ CLI subcommands against a temp file — `favorites::cli::tests::{add_defaults_to_the_cwd_and_its_name, add_refuses_duplicates_bad_profiles_and_missing_dirs, list_shape_is_stable, set_unset_move_and_rm, ambiguous_names_need_an_id, bad_usage_exits_2}`; also smoke-tested the real binary with a scratch `HOME` (add / dup exit 1 / list / --json / control-char exit 1 / usage exit 2)
3. ✓ `\n`, `\r`, ESC refused by add/set, file unchanged — `favorites::cli::tests::commands_with_control_characters_are_refused`, `favorites::tests::commands_must_be_one_plain_line`
4. ✓ Atomic writes; stale in-memory copy keeps an in-between change — `favorites::tests::writes_are_atomic_and_keep_changes_made_in_between` (no temp file left; failed write leaves old file intact), plus `app::favorites::tests::reload_keeps_surviving_links_and_rechecks_paths`
5. ✗ (manual) Click spawns in path with profile + command, click again focuses; same-name favorites distinct — not run in a GUI here. Linking by id is unit-tested (`app::favorites::tests::links_are_by_id_and_track_how_tabs_ended`).
6. ✗ (manual) Live add/rm from the CLI within ~1 s with emoji — not run in a GUI. The watcher now watches favorites.toml and reports which file changed; the emoji fallback fonts are verified by `ui::sidebar::favorites::tests::default_fonts_cover_emoji_and_symbols` (the app never replaces egui's default fonts).
7. ✗ (manual) Missing path dimmed, click toasts and spawns nothing — not run in a GUI; code path is `App::open_favorite` (re-checks on click).
8. ✗ (manual) Reset after `set <fav> command htop` runs htop — not run in a GUI; `reset_tab_now` now goes through `App::current_recipe`, which uses the favorite's current recipe while it exists and its dir exists.
9. ✗ (manual) Drag reorder, context menu, collapsed view, dialog validation, light/dark colors — not run in a GUI. Colors come only from `UiColors` (green / danger / raised_sidebar).
10. ✗ (manual) Idle CPU ~0% — not measured. No new timers or repaints: favorites reuse the existing 1 s watcher thread; existence is checked only on load, reload and click.
11. ✓ fmt, clippy, test pass; README (Features, keybinding table, actions list, palette, new "Favorites" section with CLI and file locations), `config.example.toml` and its README copy (commented `add_favorite` / `open_favorite_1` examples), help window ("Favorites" actions and gestures), `src/main.rs` crate map updated.

### Notes for reviewers
- Ids are `f<N>` from a `next_id` counter stored in the file (also bumped past any `f<N>` in the file), so they are never reused.
- Paths are stored canonicalized (on macOS `/tmp` becomes `/private/tmp`); duplicate detection compares canonical paths.
- `config::watch::Watcher::changed` now returns the changed paths instead of a bool, so a favorites edit doesn't reload the config and vice versa.
- A tab opened from a favorite keeps its normal titles (no custom title is forced); the tab's hover card says which favorite opened it.
- "Exited" is set when the linked tab's shell exits on its own (events.rs), and cleared by the next open; a user close just drops the link.

## Round 1
Commits: ac094cb Check hand-edited favorites on load and cap the pinned list's height
Checks: fmt ✓  clippy ✓  test ✓ (151 passed)
Pushed: origin/factory/003-favorite-sessions at ac094cb

### Acceptance criteria
1–4, 11. ✓ as in the initial build, plus the new tests below.
3. ✓ also enforced on load now — `favorites::tests::hand_edited_control_characters_never_load`, `app::favorites::tests::a_hand_edited_multiline_command_is_never_typed`, `favorites::cli::tests::list_escapes_control_characters_from_the_file`
5–10. ✗ manual, still not run in a GUI (unchanged from the initial build).

### Review responses
- code #1 (Windows `\\?\` paths): fixed in ac094cb. `simplify`/`strip_verbatim` in `src/favorites/mod.rs` are used by both `canonical_dir` and `same_dir`. `\\?\C:\…` becomes `C:\…` and `\\?\UNC\…` becomes `\\…`; non-drive verbatim paths are left alone. It runs on every OS (it can't match a Unix path) and is tested by `strips_windows_verbatim_prefixes`.
- code notes: `set`/Edit no longer require an unchanged path to exist (`a_missing_directory_can_still_be_edited`). `vtt favorites --help`/`-h`/`help` prints usage to stdout and exits 0 (`help_prints_usage_and_succeeds`). The sidebar doc line is re-wrapped. Esc/Enter are left to an open profile popup. "Open in split" on an open favorite focuses its tab; the README now says so.
- security #1 (control chars only checked on write): fixed in ac094cb. `Favorites::parse` sanitizes every entry: a bad command is dropped, control chars in names become U+FFFD, and a warning names the favorite. The other favorites still load. `favorite_recipe` also filters the command through `check_command` as a second guard. `list` escapes control chars in id/name/path/profile/command. The README already states the rc-file trust level. `to_toml`'s `expect` is left as is: it can't fail for these types.
- ui #1 / ux #1 (unbounded favorites section): fixed in ac094cb. The rows are in their own `ScrollArea` (`id_salt(("favorites", expanded))`, `max_height` = 40% of the remaining sidebar height, `auto_shrink([false, true])`). The drag insertion line is hit-tested and clipped against the visible area.
- ui notes: the missing-dir dim goes through `ensure_contrast(.., sidebar, max(3.0, min_contrast))`. The hover glyph is ▶ when there is no tab and ↻ when there is. The focused favorite's tint is lighter than the tab's. The palette badge stays `c.bell`: every other `UiColors` hue already has a meaning there, and a derived mix would not be clearly distinct.
- ux notes: a fixed favorites file now clears its "invalid favorites file" banner, and a config reload re-reports a still-broken one (`reload_config` calls `reload_favorites`). Declined: "`~` in the dialog isn't expanded". It already is, because the dialog's path goes through `canonical_dir`, which calls `expand_tilde`. The nine "Open favorite N" rows in the help window stay, since rebinding needs one row per action.
