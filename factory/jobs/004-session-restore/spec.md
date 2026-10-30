# Restore open tabs on launch

## Goal
When vtt starts, it brings back the tabs from last time: their order, folders, splits, titles,
directories and favorite links. Only the tab you land on starts a shell. The rest come back
dormant and start when you first focus them, so a 30-tab session costs nothing until it's used.
A `vtt session` CLI lets scripts and agents inspect or clear what was saved.

## Scope
- In: saving the session on structural changes (debounced) and on quit; restoring it at launch
  with dormant tabs; the `restore_session` config key; `vtt session show [--json] | clear | path`.
- Out (non-goals): scrollback or running-process restore; restoring file previews and the
  settings-editor tab (they have no shell recipe); the "reopen closed tab" stack; per-pane zoom;
  several named sessions; controlling a running instance (IPC); multi-window merging.

## Design
Job 1 (recipes) and job 2 (favorites) are merged on `main`. Follow `src/favorites/` as the
pattern, and build from `main`: the local `factory/board-003` checkout is behind it.

- **`src/session/mod.rs`** (new; add it to the crate map in `main.rs`): pure data and IO, no egui.
  `session.toml` in `dirs::data_dir()/vtt/`, with `version`. It holds the tabs in sidebar order.
  Each tab has a profile *name*, dir (the tab's **last known cwd**, else the recipe dir), command,
  custom title, favorite id and folder index. Then the folders (name, color, collapsed), the views
  (split tree with leaves as tab indices, ratios, focused leaf, name), and the active view.
  Atomic write (temp file plus rename). Pull the favorites `Store`'s atomic-write helper out so
  both use it, rather than copying it. A parse error or newer `version` is an `Err`, never a panic.
- **Snapshot/rebuild** are pure functions that convert between the workspace plus each tab's
  recipe fields and this data, with unit tests. Building a `Workspace` from saved data probably
  needs a constructor in `workspace.rs`. Keep it bookkeeping-only and test it there.
- **App (`src/app/session.rs`, new)**: a `dirty` flag, set wherever the structure changes (add,
  close, move, folder ops, split/minimise/ratio drag end, rename, focus change, cwd change, favorite
  link). When the flag is set, schedule a single save about 2 s out with `request_repaint_after`. No
  timer runs while nothing changes. Also save from eframe's `on_exit`/`save` so a normal quit is
  exact, and a crash loses at most the last debounce window.
- **Dormant tabs**: a new `Content::Dormant` (or equivalent) that holds the recipe and no PTY. At
  launch, every pane of the active view spawns, because they're visible. Every other tab is dormant.
  Activating or showing a dormant tab (click, switcher, next/prev, favorite click, drop into a
  split) spawns it in place from its recipe, keeping id, position, folder, title and favorite link.
  Reset on a dormant tab just spawns it, and closing one only removes it. The sidebar draws dormant
  rows dimmed using `UiColors` (no hardcoded colors). The favorite status dot shows "no session"
  for a dormant linked tab.
- **Fallbacks at restore**: an unknown profile uses the default profile and warns in the banner.
  A missing dir uses the profile's cwd, else home. A favorite id that no longer exists drops the
  link. A file that can't be read or parsed is renamed to `session.toml.bad`, with a banner warning
  and a normal fresh start. An empty or absent file gives today's single new tab.
- **Config**: `restore_session = true`. Register it in `Config`, `config/reference.rs` (so
  `vtt config get/set/unset/check/reference` know it) and `config.example.toml`. When it's off, vtt
  neither restores nor writes the file, and leaves any existing file alone.
- **CLI (`src/session/cli.rs`)**, dispatched from `main` like `favorites::cli::run_cli`.
  - `show`: one line per tab with position, active marker, title, profile, dir, command, folder,
    favorite and split membership.
  - `show --json`: a stable object `{ "active": n, "tabs": [...], "folders": [...] }`. No file
    gives an empty list and exit 0.
  - `clear`: deletes the file (exit 0 even if it's already absent).
  - `path`: prints the file's location.
  - Exit codes are 0 for ok, 1 for errors (with an actionable message), and 2 for usage.
  - Tests run against a temp file through an `Env` like the favorites CLI uses.
- **Docs**: README covers it under Features ("Session restore"), the Configuration block plus the
  example copy, and a "From the command line" paragraph for `vtt session`.
- **Keybinding**: none needed. There are no new actions.

## Acceptance criteria
1. Snapshot then rebuild round-trips order, folders (name, color, collapsed), split trees with
   ratios, view names, focused pane, active view, titles, commands and favorite links. Verify:
   unit tests in `session` and `workspace`.
2. The saved dir is the last known cwd, not the original recipe dir. Verify: unit test on
   snapshot.
3. Restore with an unknown profile, a missing dir, a removed favorite, a corrupt file and a
   newer version each behaves as described under Fallbacks. Verify: unit tests in `session`.
4. `vtt session show`, `show --json`, `clear` and `path` produce the documented output and exit
   codes, and unknown arguments exit 2 with usage. Verify: CLI unit tests against a temp file.
5. `restore_session` is known to `vtt config get/set/check/reference`, and when it's false
   nothing is restored or written. Verify: unit test plus manual: set it false, open tabs, quit,
   and check that the file is unchanged.
6. Manual: open 5 tabs in two folders with one split, `cd` somewhere in each, quit, relaunch.
   Layout and directories come back, only the active view's shells run (`ps`), the others are
   dimmed, and clicking one starts its shell in its saved dir.
7. Manual: `kill -9` vtt a few seconds after a structural change. On relaunch the change is
   there.
8. Manual: idle CPU stays about 0% with a restored session, and no repaint happens while nothing
   changes.
9. `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`
   pass, and README and `config.example.toml` are updated.

## Risks
- **Deviation from the plan's "the app watches the state file"**: the session file mirrors the
  running app, so the app doesn't watch or reload it. `vtt session clear` while vtt is running is
  overwritten by its next save. Document that: quit first, then clear. Live control needs IPC,
  which is out of scope.
- **Several vtt instances**: last writer wins. Document it, and don't add locking in this job.
- **Debounce versus durability**: cwd changes arrive from the existing poll. Mark the snapshot
  dirty only when a cwd actually changed, so a busy shell doesn't cause repeated writes.
- **Windows**: the cwd isn't known there (procinfo is Unix), so the saved dir falls back to the
  recipe dir. `rename` over an existing file must work on Windows. The favorites helper already
  handles that.
- **Security**: the file stores commands that get typed into shells at focus, and it lives in
  the user's own data dir. Nothing runs until the tab is focused, and restore never types a
  command into a fallback profile it had to substitute. Open question: should restored tabs
  re-type their command at all, or start only the shell? The default proposed here is yes,
  matching Reset.
