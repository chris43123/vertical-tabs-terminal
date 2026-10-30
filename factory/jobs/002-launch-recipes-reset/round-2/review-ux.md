VERDICT: PASS

## Blocking

## Notes
- Round 1 notes: the respawn-failure note is fixed. `reset_tab_now` now starts the new session first and, on failure, leaves the tab and its old session in place with the error in the problems banner. The other round-1 notes (preview no-op, background job at an idle prompt, Windows never asks, README bullet length) were non-blocking and are unchanged.
- New default `cmd+shift+r` on macOS only, chosen by the human. Linux/Windows stay unbound, with no Alt chord. A Cmd chord is consumed by vtt and never reaches the shell. It sits beside the existing macOS-only `cmd+r` (rename) and `cmd+shift+t` (reopen). The test `reset_tab_is_bound_on_macos_only` and `default_tables_parse_on_every_os` pass. The `ctrl+shift+r` example is still checked against the Linux/Windows defaults.
- Docs match: the README Features bullet, the Managing tabs table (`⌘⇧R`), the README config copy and `config.example.toml` all say unbound on Linux/Windows and `cmd+shift+r` on macOS. Help takes the chord from the keymap.
- One chord now resets an idle tab with no prompt, even with `sleep 1000 &` at the prompt, because the confirm check is `shell_idle()`. That follows the spec and the README says background jobs are killed. Consider a README clause saying a background job at an idle prompt is killed without asking.
- The reopen change (only previews take `closed.custom_title`) keeps Reopen's behavior for terminal tabs, since the recipe already carries the title.
