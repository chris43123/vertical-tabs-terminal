VERDICT: PASS
Reviewed: 7d6bfc0

## Blocking

## Notes
- Reviewed 4cc3da7..7d6bfc0 (round 1 passed 4cc3da7). The only security-relevant change is `App::wake` setting `pending_input = None`. That removes the one path where a saved command from `session.toml` was typed into a shell at first focus, so it shrinks the surface. A tampered session file can no longer run anything on focus.
- The saved command stays in the recipe, so an explicit Reset on a running tab still types it. That is the user's own action, as before. The substituted-profile case still drops the command in `rebuild`. The doc comment there ("not even Reset types it") matches the code.
- The other changes (a shared `DIM_CONTRAST` constant, a dimmed title color, README text) touch no process, path, parse or unsafe code. No new dependencies.
