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

## Human feedback (2026-10-30T17:07Z)
Commits: 8abc5cf Make a favorite's row its tab and let one directory be pinned twice
Checks: fmt ✓  clippy ✓  test ✓ (152 passed)
Pushed: origin/factory/003-favorite-sessions at 8abc5cf

### Acceptance criteria
Unchanged from round 1. The manual criteria are being run by the human.

### Review responses
- human #1 (README, dropped bad command): done. The Favorites section's file paragraph now says a hand-edited command with a line break or control character is ignored with a warning and dropped from the file on the next write, next to the rc-file trust note.
- human #2 (favorite row is its tab): done.
  - The tab list skips every tab under a favorite (`App::tabs_under_favorites`). The grouping is the pure function `group_favorite_tabs` in `src/app/favorites.rs`, tested by `favorite_rows_take_their_tab_and_its_split`.
  - The favorite row has the tab-row highlight (focused / active view), the bell and unread dots, × close on hover, middle-click close, and the split bracket.
  - Its menu, when open: Reset, Split right with new tab, Minimise from split, Reveal in files panel, Rename / edit…, Close tab, Remove favorite. Rename goes through the edit dialog, since the name lives in the file. Double-click opens the same dialog.
  - Split siblings are drawn as normal tab rows under the favorite.
  - A favorite tab opened without a split leaves any folder. `sync_favorite_order` keeps the favorites' tabs at the front of `ws.order`, in favorites order, so next/previous tab and "go to tab N" match the sidebar.
  - Side effect to know: "Open in split" on a closed favorite splits the focused view, so that view's tabs move under the favorite. That follows from "splits stay with it".
- human #3 (resizable section): done. A drag handle (the line under the section, resize cursor) sets `App::favorites_height`, clamped between one row and the rows' full height, and at draw time to at most 85% of the remaining sidebar. Double-click goes back to the default (40% cap). The height is persisted with eframe storage (`favorites_height`), like the UI zoom.
- human #4 (same directory twice): done. The duplicate-path check is removed from `validate` (and `same_dir` is gone). The tests now assert that a second favorite for the same path gets its own id (`same_dir_twice_is_allowed_…`, `add_allows_one_dir_twice_…`).
- human #5 ("Favorites" header with + on hover): done. It shows in the expanded sidebar, including when the list is empty (so the + can add the first one). The collapsed strip shows nothing until there are favorites.

## Round 3
Commits: 808dcbc Rename a favorite's tab through its edit dialog
Checks: fmt ✓  clippy ✓  test ✓ (152 passed)
Pushed: origin/factory/003-favorite-sessions at 808dcbc

### Acceptance criteria
Unchanged. Manual criteria are with the human.

### Review responses
- design #1 (Rename tab on a favorite's tab does nothing and keeps the peek open): fixed in 808dcbc.
  - `start_rename` opens the favorite's edit dialog when the tab is a favorite's own tab (`FavoritesState::favorite_for_tab`).
  - `start_view_rename` does the same for a split view under a favorite.
  - As a guard, `App::drop_undrawn_rename` runs every frame before the sidebar or zen overlay and clears a rename whose target has no drawn editor.
  - Test: `App` can't be built in unit tests (it needs an egui/eframe context), so the test covers the lookup the fix relies on: `links_are_by_id_and_track_how_tabs_ended` now asserts `favorite_for_tab` for linked, unlinked and unknown tabs. That `renaming` stays `None` is left for the manual pass (Cmd+R on a favorite's tab).
- code note (reopen_closed_tab): fixed in 808dcbc. A reopened tab that relinks to its favorite leaves its folder and `sync_favorite_order` runs.
- Left as notes: `favorite_tabs()` runs twice per frame (cheap at realistic sizes). `sync_favorite_order` doesn't run when a tab is later split under a favorite. "Open in split" keeps the group (documented side effect).

## Human feedback (2026-10-30T20:36Z)
Commits: 71f23fb Show favorites as tiles and split into a favorite as a plain tab
Checks: fmt ✓  clippy ✓  test ✓ (154 passed)
Pushed: origin/factory/003-favorite-sessions at 71f23fb

### Acceptance criteria
Unchanged. The manual criteria are with the human (round-4 pass reported 5, 8, 10 working).

### Review responses
- human #1 (double-click on the resize handle didn't reset): fixed in 71f23fb. The handle was allocated with `Sense::drag()`, which never reports clicks, so `double_clicked()` was always false. It now uses `Sense::click_and_drag()`. Manual check: double-click the line under the favorites.
- human #2 (Open in split of a favorite into another favorite's view): fixed in 71f23fb. `open_favorite` checks whether the focused tab (the split target) is a favorite's own tab or one of its split's tabs (`in_favorite_split`, tested by `a_split_into_a_favorite_is_found_from_any_of_its_tabs`). If so, the recipe's `favorite` link is cleared. The new tab is a plain tab with the favorite's cwd, profile and command, listed under the target favorite, and the opened favorite's row stays closed. Because it isn't linked, Reset on that tab restarts its own recipe rather than following later edits to the favorite. A favorite split into a normal tab's view is unchanged. README documents it.
- human #3 (same indicator in both widths): done in 71f23fb. `favorite_tile` draws, in both widths, the rounded square with the first grapheme, the status dot on its bottom-right and the unread/bell badge on its top-right. Expanded it is 20 px, followed by the name. A leading emoji or symbol isn't repeated in the name (`row_label`, tested by `row_label_drops_the_emoji_the_tile_shows`). The tooltip still shows the full name. The badge moved from the row's right end onto the tile. README and the help window ("Tile dot") are updated.
- Known non-blocking leftovers, unchanged as the human said: stale "command dropped" banner, no auto-scroll while dragging, `sync_favorite_order` after a later split, unescaped ids in `diag::warn`.
