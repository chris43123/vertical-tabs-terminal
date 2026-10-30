VERDICT: PASS
Reviewed: 7d6bfc0

## Blocking

## Notes
- Round 2 had no blocking points. Only the change since d724dd7 was reviewed: `App::wake` no longer sets `pending_input`, plus the README bullet and two doc comments.
- Every route that starts a restored tab (visible at launch via `wake_visible`, or woken on first focus) goes through `wake`, so the new behaviour is uniform: shell only, in the last known dir, nothing typed. A fresh new tab and an explicit Reset on a running tab still type the recipe command (`spawn_tab`, reset path), matching the README wording.
- README "Session restore" bullet says what changed in plain style and matches the code. `config.example.toml` already says running programs aren't restored, so it needs no edit.
- No chrome changed in this round, so part 2 has nothing new to check.
- For the manual pass: the human already reported the manual checks OK. One thing to glance at: a favorite whose recipe has a command (e.g. `claude`) now wakes to a bare shell, so its status dot will show a plain shell, not the program. That is intended by the feedback.
