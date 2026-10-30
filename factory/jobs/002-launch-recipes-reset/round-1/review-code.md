VERDICT: PASS

## Blocking

(none)

## Notes
- Checks run in the worktree: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (120 passed) are clean. `cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings` is also clean, so the `windows-sys` Job Object code compiles with the added feature flags. No new crate: `Cargo.lock` only gains the `windows-sys 0.59.0` edge.
- Criteria 1-4 and 10 are covered by tests or checks I could run: the recipe tests, `typed_input_is_command_and_carriage_return`, `reset_tab_is_an_unbound_action`, and the existing reference/example tests. The README, `config.example.toml`, help section, palette and context-menu entries are all in place. Criteria 5-9 are GUI-only and build.md marks them as not run; that is acceptable here, but the human reviewer should do them (especially 6: `sleep 1000 &` plus `vim`, then Reset).
- Design matches the spec: the generation tag via `Source` is filtered at the top of the `process_events` loop, so the old shell's `ChildExit`/`Exit` can't close a reset tab. The command is written on the first `Wakeup`, not on a timer. `reset_tab_now` swaps `tab.content` under the same id. SIGKILL escalation is a single one-shot thread, so there is no polling. The modal draws only while open, and `apply_motion` already covers `reduce_motion` for egui's modal animation. The `initialized` guard stops the Enter that opened the prompt from also answering it. `overlay_open()` is a good dedupe of the three repeated checks in `input.rs`.
- `src/app/tabs.rs` `close_tab`: a closed tab's recipe now carries the last seen cwd as `dir`, so a tab reopened from the closed list resets to that directory rather than the original one. This matches the old reopen behaviour and is fine for job 1. Job 3 (session restore) may want the original recipe kept apart from the last cwd.
- `src/app/tabs.rs` `reopen_closed_tab`: `custom_title` is set twice, once through `recipe.custom_title` in `spawn_tab` and again from `closed.custom_title`. `ClosedTab.custom_title` is still needed for previews, but the second assignment could be limited to that case.
- `src/terminal/session.rs` `kill::session`: if the shell is already gone and its pid was reused as an unrelated session or group id, `killpg(sid)` could hit an unrelated group. The same applies to the 500 ms delayed re-scan. `groups_in_session` only matches live members and the window is tiny, so it is not worth blocking. A cheap guard would be to skip `kill_tree` when the child has already exited.
- On a spawn failure during Reset, the old session has already been signalled and stays in place; its exit then closes the tab. This is documented in build.md and the code comment. Showing the failure in the banner is consistent with `spawn_tab`.
- README's config copy was already behind `config.example.toml` (build.md says so). Only the new key was added there, which is the right scope.
- The generation filter in `events.rs` has no unit test, because it needs a full `App`. If a small pure helper existed (for example `fn is_current(session_generation: Option<u64>, source: Source) -> bool`), it could be tested directly. Optional.
