# Launch recipes and Reset tab

## Goal
Every terminal tab remembers how it was launched (its launch recipe), and a new
**Reset tab** action kills everything running in the tab and starts it again from that
recipe in the same slot. A wedged REPL, a hung `ssh` or a runaway build is one click
from a clean shell, and favorites (job 2) and session restore (job 3) build on the recipe.
This is job 1 of `factory/plans/sessions-and-favorites.md`.

## Scope
- In: a `LaunchRecipe` on every terminal tab; spawning from a recipe (an optional
  command is typed into the shell); killing a tab's whole process tree; resetting in
  place; confirming when something is running; one config key; palette and tab-menu entries.
- Out (non-goals): favorites and the recipe's favorite link (job 2 adds the field and
  "use the favorite's current recipe"); session files (job 3); any UI for setting a
  recipe command; resetting preview tabs (no entry shown for them); `vtt reset` from
  the CLI (needs IPC, per the plan); restoring scrollback.

## Design
- **Recipe** (`src/app/tab.rs`, or a small `src/app/recipe.rs`): `profile` (name +
  resolved `Profile`), `dir: Option<PathBuf>` (resolved at spawn: requested cwd, else the
  profile's cwd, else home, so a reset is deterministic), `command: Option<String>`,
  `custom_title: Option<String>`. Pure data, `Clone`, unit-tested. Don't add an unused
  favorite field now: `-D warnings` rejects dead code.
- **Spawning** (`src/app/tabs.rs`): `spawn_tab` takes a recipe and stores it on the `Tab`.
  `new_tab`, `new_tab_in`, `duplicate_tab` and `reopen_closed_tab` build one
  (`ClosedTab` keeps the recipe). If there's a command, the spawn writes `command + "\r"`
  to the PTY once the shell is ready: on its first output or prompt, not on a timer.
  The process stays the shell, so aliases, PATH and history work.
- **Process-tree kill** (`src/terminal/session.rs`, platform code behind `cfg`): a
  `Session::kill_tree()`. Unix: the shell is a session leader (setsid), so signal the
  PTY's foreground group (`tcgetpgrp`) and the shell's group with `killpg` (SIGHUP, then
  SIGKILL for anything that ignores it, via a one-shot timer, never a loop), then drop the
  session. Windows: put the child in a Job Object with `KILL_ON_JOB_CLOSE` and terminate
  the job. That needs `windows-sys` features, which alacritty already pulls in (no new crate).
- **Reset** (`App::reset_tab(id)` in `src/app/tabs.rs`): kill the tree, spawn a new
  `Session` from the recipe and swap `tab.content` **in place under the same `TabId`**.
  Position, split, group, custom title and zoom are kept; `osc_title`, `auto_title`,
  `process`, activity and bell are cleared. The old session's late `ChildExit`/`Exit`
  must not close the tab. Tag the `Listener` with a per-session generation and drop
  stale events in `src/app/events.rs`.
- **Confirmation**: if `!session.shell_idle()` and `confirm_reset` is on, show a small
  modal ("Reset <title>? <process> is still running." Reset / Cancel). Colors come from
  `UiColors`. Enter confirms, Esc cancels. It honors UI zoom and `reduce_motion`.
- **Action**: `reset_tab` ("Reset tab") in `src/config/keybinds.rs`, **unbound on every
  platform** (it's in the palette only). It acts on the focused pane. Add "Reset" to
  the tab context menu in `src/ui/sidebar/tab_row.rs` (terminal tabs only), and add
  the action to a section in `src/ui/help.rs`.
- **Config key**: `confirm_reset` (bool, default `true`) in `Config`, in `OPTIONS` in
  `src/config/reference.rs` (so `vtt config get/set/unset/check/reference` know it),
  in `config.example.toml`, and in README's copy of it.
- **Docs**: README Features bullet, a "Reset tab | — | —" row, `reset_tab` in the
  actions list, and a `# reset_tab = "ctrl+shift+r"  # unbound by default` example
  in `config.example.toml`. Check that it doesn't clash with a default first.

## Acceptance criteria
1. Building a recipe resolves `dir` (requested, else profile cwd, else home) and keeps
   profile, command and custom title. Verify: unit tests in the recipe module.
2. A recipe with a command produces exactly `command + "\r"` for the shell, and none
   without one. Verify: unit test.
3. `vtt config set confirm_reset false` / `get` / `unset` / `check` work, and
   `vtt config reference` lists it. Verify: the existing reference tests
   (`every_config_field_is_documented`, `every_option_is_in_the_example`) pass.
4. `reset_tab` parses as an action, has no default chord on any platform and appears in
   the palette. Verify: unit test in `keybinds.rs`; manual: palette search "reset".
5. Resetting an idle tab with no confirmation leaves it in the same sidebar slot, split
   and folder with the same custom title, and the shell is fresh in the recipe's
   directory. Verify: manual: `cd /tmp`, rename the tab, Reset → prompt in the original dir.
6. With `sleep 1000 &` and then `vim` running, Reset asks first. After confirming,
   neither `vim` nor the `sleep` survives (`ps`/`pgrep` from another tab), and the tab
   stays open. Verify: manual on macOS/Linux; on Windows check that a child `ping -t` dies.
7. With `confirm_reset = false`, no dialog appears. Verify: manual.
8. Duplicate tab and Reopen closed tab still behave as before. Verify: manual.
9. Idle CPU stays ~0% (no new repaint or poll loops). Verify: manual, Activity Monitor/top.
10. `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and
    `cargo test` pass. README and `config.example.toml` are updated.

## Risks
- **Typeahead**: some shell setups (zsh plugins, p10k instant prompt) drop input written
  before the first prompt. That's why the command is written on first output, not at spawn.
  Job 1 has no UI for setting a command, so the end-to-end check comes in job 2.
- **Stale exit events** would close the freshly reset tab. The generation tag is required.
- **Windows**: `shell_in_foreground` always returns true there, so the dialog never
  shows on Windows. That's acceptable; note it in the README. Job Objects also need care
  with ConPTY: alacritty spawns the child, so the job is assigned right after spawn,
  and a grandchild can slip out in that short window.
- **Unix**: processes that call `setsid` themselves (daemons, `nohup ... &` with
  `setsid`) leave the session and survive. That's by design, document it. SIGKILL
  escalation needs a short delay and must not block the UI thread.
- **Security**: the recipe command is typed verbatim into a shell. In job 1 it only
  comes from vtt itself; job 2 must treat the favorites file as trusted user input.
- **Conventions**: the plan's "no default keybinding" also applies on macOS, so `reset_tab` has no ⌘
  default either. No conflict otherwise.
