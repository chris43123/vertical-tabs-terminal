# Favorite directories as pinned sessions

## Goal
Pin the directories you return to (each with an optional profile and command such as
`claude` or `k9s`) as rows above the tabs in the sidebar. One click opens the session
or focuses it if it's already open. The same list can be managed with `vtt favorites`,
so scripts and AI agents can curate it without the UI. This is job 2 of
`factory/plans/sessions-and-favorites.md`. Job 1 (launch recipes, Reset) is merged.

## Scope
- In: a per-machine favorites file plus its model, the `vtt favorites` CLI, live
  reload, a pinned sidebar section (expanded and collapsed), click/reset/context menu,
  drag reorder, an add/edit dialog, add entry points (tab menu, files panel folder menu,
  palette), the palette's "Open favorite: …", the unbound actions `add_favorite` and
  `open_favorite_1`…`9`, and the favorite link on `LaunchRecipe` so Reset uses the
  favorite's current recipe.
- Out: controlling a running vtt from the CLI (`vtt open <fav>`, needs IPC), session
  restore (job 3), color emoji and ZWJ sequences, syncing favorites through the config,
  polling path existence.

## Design
- **Model + file**: new module `src/favorites/` (`mod.rs` for the model and IO, `cli.rs`
  for the CLI), with no egui. It sits between `workspace` and `app` in the crate map in
  `src/main.rs`. A `Favorite` has `id` (a stable short string, unique in the file, never
  reused), `name` (emoji allowed), `path`, `profile: Option<String>` and
  `command: Option<String>`. The list order is the display order. The file is
  `dirs::data_dir()/vtt/favorites.toml`: human-readable TOML, an array of tables with a
  version key. Writes are atomic (temp file in the same dir, then rename). Every mutation
  re-reads the file, applies the change, then writes, so the app never overwrites a CLI
  edit. Pure functions: resolve `<fav>` (1-based position, then exact id, then exact name;
  an ambiguous name is an error that lists the matching ids), add (duplicate canonical
  path → error), set/unset a field, move, rm.
- **Command validation** (the follow-up from job 002's decision): `command` must be one
  line with no control characters (`\r`, `\n`, ESC, …). It is rejected on save in both
  the CLI and the dialog, so `typed_input` only ever sends one Enter. `profile` must name
  a known profile (warn, don't fail, if it's unknown at load time; fall back to the
  default profile). The path must exist when it's saved.
- **CLI** (dispatched from `main.rs` next to `config::reference::run_cli`, same style and
  usage text): `vtt favorites list [--json] | add [path] [--name N] [--profile P]
  [--command C] | set <fav> <field> <value> | unset <fav> <field> | move <fav> <pos> |
  rm <fav> | path`. `add` defaults the path to the cwd and the name to the directory's
  base name. Exit codes: 0 ok, 1 error (message on stderr says what to do), 2 usage.
  `--json` prints a stable array of objects with `id`, `name`, `path`, `profile`,
  `command`, `position` and `exists`.
- **App**: `App` holds the loaded `Favorites` and a link map `FavId → TabId`.
  `LaunchRecipe` gains `favorite: Option<FavId>`. The favorites file joins the existing
  `config::watch::Watcher` paths. On change, reload, keep links whose id survives, and
  re-check path existence. Existence is checked at load, on reload and on click only.
  Click: linked tab alive → focus it; path missing → toast, spawn nothing; otherwise
  spawn from the favorite's recipe and link it. A favorite's tab closes when its shell
  exits (current behaviour), so "exited" means the link's last tab ended on its own
  rather than by the user closing it: show the exited dot, and the next click spawns
  again. `reset_tab_now` uses the favorite's current recipe when `recipe.favorite` still
  resolves.
- **UI**: `src/ui/sidebar/favorites.rs` draws the pinned section above the tabs. Each
  row has a status dot (running / no session / exited, colored with `UiColors`), the name
  ellipsized with a tooltip showing the full name and path, a ↻ reset button on hover,
  and a dimmed look when the path is missing. Collapsed sidebar: the first grapheme plus
  the dot. Rows reorder by drag (writes the file). Context menu: Open, Open in split,
  Reveal in files panel (`browse_files`), Edit…, Remove. Add entry points: the tab menu's
  "Add directory to favorites" (cwd), the files panel folder menu, and the palette.
  `src/ui/favorite_dialog.rs` is a small modal for name, path (prefilled, checked on
  save), a profile picker and the command. Duplicate path or a bad command → inline
  warning and no save. This edits vtt's own state, never user files.
- **Keybindings**: `add_favorite` and `open_favorite_1`…`9` (numbered like `GotoTab`),
  all unbound on every platform, all in the palette. Add a commented example to
  `config.example.toml`.
- **Docs**: README Features, the keybinding table plus actions list, a "Favorites"
  section with the CLI examples and the file location; `config.example.toml` (examples
  only, no new config key); help window; `src/main.rs` crate map.

## Acceptance criteria
1. Resolving `<fav>` by position, id and name works, and an ambiguous name errors with
   its ids — verify: unit test in `favorites`.
2. Each CLI subcommand works against a temp file: list/`--json` shape, add (cwd default,
   duplicate path refused with exit 1), set/unset, move, rm, and exit code 2 on bad
   usage — verify: unit tests in `favorites::cli`.
3. A command containing `\n`, `\r` or ESC is refused by `add`/`set` and the file is
   unchanged — verify: unit test.
4. Writes are atomic, and a mutation made on a stale in-memory copy keeps a change
   written to the file in between — verify: unit test.
5. Clicking a favorite with no tab spawns one in its path with its profile and types its
   command, and clicking again focuses that same tab (two favorites with the same name
   stay distinct) — manual.
6. `vtt favorites add /tmp --name "🚀 tmp"` while vtt runs adds the row within ~1 s with
   the emoji visible (monochrome), and `rm` removes it — manual.
7. Missing path: the row is dimmed, a click shows a toast and spawns nothing — manual
   (`mkdir`, add, `rmdir`, then click).
8. Reset on a favorite's tab after `vtt favorites set <fav> command htop` runs `htop` —
   manual.
9. Drag reorder, the context menu entries, the collapsed view and the dialog validation
   behave as described; colors follow light and dark themes — manual.
10. Idle CPU stays ~0% with favorites shown — manual (Activity Monitor).
11. `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and
    `cargo test` pass, and README, `config.example.toml` and help are updated.

## Risks
- The "event-driven" watch reuses the existing 1 s mtime-stat thread. That is existing
  precedent with negligible CPU, but it is not truly event-based. Two writes within the
  mtime granularity could be missed. Re-reading before every write covers correctness.
- `data_dir` equals `config_dir` on macOS, so the file sits next to `config.toml` there.
  It is still a separate, unsynced file. Windows and Linux paths differ; document all
  three.
- Security: `command` is typed into a shell, so anything that can write the file can run
  commands as the user. That is the same trust level as the shell rc files. The
  single-line check stops multi-command injection by newline, not malicious single
  commands. A CLI driven by an agent is the expected writer.
- egui's default fonts include a monochrome emoji fallback. Check that the UI font setup
  hasn't dropped it. Graphemes made of several code points may render as separate boxes
  (out of scope).
- Open question: the plan's "exited → relaunch in place" assumes exited tabs stay open,
  but vtt closes them. This spec maps it to "show exited, spawn a new tab on click".
  Keeping a dead tab open would be a separate change.
