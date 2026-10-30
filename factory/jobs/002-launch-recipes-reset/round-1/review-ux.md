VERDICT: PASS

## Blocking

## Notes
- src/app/tabs.rs (reset_tab_now) — `kill_tree()` runs before the new session is started. If the respawn fails (shell removed, bad profile), the old session is already dead, its exit closes the tab, and the only trace is the problems banner. Starting the new session first and killing the old one after the swap would keep the tab on failure. Rare, so not blocking.
- Reset from the palette while a preview tab is focused is a silent no-op (`reset_tab` returns early). The tab menu hides the entry for previews, as the spec says. A toast or banner line ("Only terminal tabs can be reset") would be friendlier, but it is not required.
- The confirm check is `shell_idle()`, so an idle prompt with `sleep 1000 &` resets without asking, and the README says background jobs are killed. This follows the spec and the README states it. A short README clause like "a background job at an idle prompt is killed without asking" would be more honest.
- On Windows the dialog never shows (documented in README and `config.example.toml`), so Reset there kills a running program without asking. This is accepted in the spec.
- Keybinding rules hold: `reset_tab` is unbound on every platform, it is in `ACTIONS` (palette, help "Tabs" section, `vtt config reference`), and the example `ctrl+shift+r` is checked in a test not to clash with a default. No Alt chords were added.
- The prompt does not steal keys from the shell. `overlay_open()` stops forwarding to the terminal only while the modal is open. The `initialized` guard stops the Enter that opened it from also answering it. Enter and Esc are consumed, so they don't leak to the PTY afterwards.
- `confirm_reset` is optional with default true, is read from `self.config` at use time (so it hot-reloads), and is in `OPTIONS`, `config.example.toml` and the README config copy.
- README: the Features bullet is long next to its neighbours. Consider splitting the `setsid` caveat into a second sentence or a "Notes" line. It is otherwise in the README's plain style.
- Manual acceptance checks 5, 7 and 8 were not run, as build.md says. Someone should run them in the GUI (reset keeps slot and title, no dialog with `confirm_reset=false`, Duplicate and Reopen unchanged). The code paths read correctly.
