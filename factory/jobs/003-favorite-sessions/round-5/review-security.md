VERDICT: PASS
Reviewed: 337bbca

## Blocking

## Notes
- Split-into-a-favorite path (src/app/favorites.rs `open_favorite`): the new tab is built by the same `favorite_recipe` as before (profile looked up by name, path from the favorite, command re-checked with `check_command`, one line, typed with one Enter). The only change is `recipe.favorite = None`, so it is a plain tab. Nothing new reaches the PTY: no new argv/env/escape input.
- Reset on that tab: `current_recipe` only swaps in the favorite's current recipe when `recipe.favorite` is set; with `None` it replays the tab's own stored recipe, the same command, dir and profile it started with. It does not re-read the favorites file, so a hand-edited command there can't reach an unlinked tab through Reset. `reset_tab` still goes through the `confirm_reset` prompt when something runs. Reset for a linked tab still needs the favorite's dir to exist (`is_dir`) and re-validates the command.
- `link_favorite` is skipped for the unlinked recipe, so no stale link or double favorite row. A favorite removed meanwhile just falls back to the tab's own recipe.
- Allowing one directory pinned twice removes the duplicate check only; the path is still canonicalized and checked to be a directory, so no path-handling regression.
- `favorites_height` read from eframe storage: a NaN, negative or huge value cannot panic (`min`/`max` on NaN return the other operand, and the drag `clamp` has valid bounds). The value is a UI size only; nothing secret is stored.
- Autoscroll for favorite drags reuses `ui::preview::autoscroll`: no filesystem or process effect.
- No new dependencies, no `unsafe`, nothing logged or written beyond the f32 above.
