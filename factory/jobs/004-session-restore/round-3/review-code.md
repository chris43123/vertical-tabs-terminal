VERDICT: PASS
Reviewed: 7d6bfc0

## Blocking

## Notes
- Only 7d6bfc0 changed since my last review (d724dd7). It matches the human feedback in human.md: `wake` sets `pending_input = None`, so every restored tab starts only its shell in the last known dir. Visible panes at launch go through `wake_visible` -> `wake`, so they follow the same rule.
- `typed_input` is still used by `tabs.rs` (new tab and Reset on a running tab), so nothing is left dead. The recipe command stays in the session file, so the round-trip tests are unaffected.
- README "Session restore" bullet and the `wake` and `rebuild` doc comments are updated and agree with the behavior. The commit message follows the conventions.
- Checks in the worktree are clean: fmt, clippy `-D warnings`, test (176 passed).
- Nit: the reworded `rebuild` doc comment line ("shell the user didn't choose; a missing directory ...") is now longer than the lines around it. Not worth a change.
- No unit test for the `pending_input = None` line, as build.md says. `wake` needs a live App and PTY, so I accept that.
