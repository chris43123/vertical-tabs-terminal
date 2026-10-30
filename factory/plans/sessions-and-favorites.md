# Plan: launch recipes, tab reset, favorites, session restore

Agreed with the user on 2026-10-29. Three backlog jobs build it, in order:

1. Launch recipes + reset tab
2. Favorites (needs 1)
3. Session restore (needs 1; independent of 2)

Written before the latest UI changes were synced, so it names responsibilities, not
files or types. Read the current code (crate map at the top of `src/main.rs`) and map
each point to whatever exists today.

**Before starting job 2 or 3, check that job 1 is merged into `main`. If it isn't,
the builder replies BLOCKED.**

## Cross-cutting requirement: everything works from the vtt CLI

- New settings are ordinary config keys, registered wherever `vtt config` learns its
  keys (so `get`/`set`/`unset`/`check`/`reference` know them), plus
  `config.example.toml` and the README.
- Per-machine state (favorites, session) is not config. It gets its own subcommands,
  following the style of `vtt config`. Commands that list things take `--json`, use
  clear exit codes and print actionable errors, because AI agents run them.
- That state lives in its own human-readable file in the app's data directory
  (per machine, never synced through the config). The CLI edits it whether or not vtt
  is running. The app watches it, event-driven like the config watcher, so CLI changes
  appear live with no polling. Both sides write atomically (temp file + rename), and the
  app re-reads the file before writing so it never overwrites a CLI change.
- Each subcommand has tests against a temp file, without the UI.
- Out of scope: controlling a running instance from the CLI (`vtt open <fav>`,
  `vtt reset`). That needs IPC and comes later.

## Job 1: launch recipes + reset tab

- Every tab keeps its **launch recipe**: shell profile, directory, optional command,
  custom title, and the favorite it came from (if any).
- A recipe's command is **typed into the profile's shell** after it starts, not run as
  the process. That way aliases and PATH work, it lands in history, and quitting it
  leaves a shell in that directory.
- **Reset tab** kills the tab's whole process tree (process group on Unix, job object
  or equivalent on Windows) and respawns from the recipe in the same slot, keeping
  position, title, group and favorite link. If the tab came from a favorite that still
  exists, use the favorite's **current** recipe; otherwise use the tab's original one.
- Ask for confirmation only when something other than the idle shell is in the
  foreground (vtt already knows the foreground process). Config key to turn that off.
- Action in the palette and the tab context menu. **No default keybinding** (see
  conventions); add an example in `config.example.toml`.

## Job 2: favorites

- A favorite is a named launch recipe: name (emoji allowed), path, optional profile,
  optional command (`claude`, `k9s`…). It has a stable id, since names can repeat or
  contain spaces and emoji.
- Clicking a favorite: no linked tab → spawn one from the recipe; linked tab alive →
  focus it; linked tab exited → relaunch it in place. The link is favorite → tab id,
  never "some tab whose cwd matches".
- Sidebar: a pinned section above the tabs, **one row per favorite** with a status dot
  (running / no session / exited, colors from the theme), the name (ellipsized, full
  name and path in a tooltip), and a ↻ reset on hover. Collapsed sidebar: first
  letter or emoji plus the dot. Reorder by drag.
- Context menu: open, open in split, reveal in files panel, edit, remove. Add from
  the tab context menu ("Add directory to favorites"), the files panel folder menu, and
  the palette. A small dialog edits name, path (prefilled with cwd, checked on save),
  profile and command.
- A missing path shows the row dimmed; clicking it shows a toast and spawns nothing.
  Existence is checked at load and on click, never polled.
- Emoji: monochrome emoji through the UI's fallback font is enough; check the font
  setup still includes it. Color emoji and ZWJ sequences are out of scope.
- Duplicate path → warn, don't add.
- Palette: "Open favorite: …" (fuzzy). Unbound actions `open_favorite_1`…`9`, `add_favorite`.
- CLI: `vtt favorites list [--json] | add [path] [--name] [--profile] [--command] |
  set <fav> <field> <value> | unset <fav> <field> | move <fav> <pos> | rm <fav>`,
  where `<fav>` is a position, id or name.

## Job 3: session restore

- Save the open tabs' recipes (using the **last known cwd**, not the original one),
  titles, order, groups, active tab, favorite links and, if the workspace model allows,
  the split layout. Save on every structural change, debounced, and on quit, so a crash
  still recovers.
- On launch, only the active tab spawns. The others come back **dormant** (dimmed in
  the sidebar) and spawn when first focused. A dead process doesn't matter: the tab is
  rebuilt from its recipe. Scrollback is not restored.
- Config: `restore_session` (default on).
- CLI: `vtt session show [--json] | clear`.
