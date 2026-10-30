## 2026-10-30T16:50:16Z

Decisions on decision.md "For the human":
1. Reset follows later edits to the favorite (current recipe, profile included): accepted, as in the plan.
2. favorites.toml format and location (next to config.toml on macOS): accepted.
3. Windows-only path handling tested only as string logic: accepted as is; Windows is not a priority for now.
4. A hand-edited bad command is dropped on the next write: document it in the README (favorites section), next to the note that anything able to write the file can run commands.

Manual criteria 5-10: the human is running them now; results to follow.
## 2026-10-30T17:07:24Z

Manual results on macOS: criteria 1, 3, 4, 6, 7 of the list work. Changes wanted:

1. README (decision 4 above): document that a hand-edited bad command is dropped on the next write.
2. A favorite's tab should BE the favorite's row, not a second row in the normal tab list. Opening a favorite must not add a row among the regular tabs; the favorite row itself acts as that tab (selected/focused state, activity/bell/exited markers, rename and the rest of the tab context menu where it makes sense, close). Splits made from it stay with it. Today it works but having the tab appear next to the normal ones feels odd.
3. The favorites section's height should be resizable with the mouse: a drag handle between the favorites section and the tab list (keep the current cap as the default; remember the chosen height for the session at least).
4. Allow the same directory more than once with different commands (e.g. a plain terminal and k9s in the same path, different purposes). Drop the duplicate-path refusal; favorites are distinct by id. Clicking each one opens/focuses its own tab.
5. UI: a "Favorites" header label above the section, with a "+" button that only appears on hover and opens the add-favorite dialog.

## 2026-10-30T20:36Z

Manual pass of the round-4 build on macOS: favorite row as its tab, Cmd+R (favorite tab, split sibling, collapsed sidebar), Cmd+Shift+T reopen and next/prev/go-to-tab order, and criteria 5, 8, 10 (`vtt favorites add /tmp --name "🚀 tmp"`, `set 1 command htop` + Reset, idle CPU) all work. Changes wanted:

1. Bug: double-clicking the favorites resize handle does not reset the height to the default. Dragging it works.
2. "Open in split" of one favorite into another favorite's view: today both favorite rows get a group color strip, but the strips aren't continuous (the rows are apart), which looks wrong. Instead, when the split target is a favorite, open a normal terminal with that favorite's recipe (cwd, profile, command) and group it with the favorite whose view it splits; the second favorite's own row stays untouched. Grouping a favorite with normal tabs works well as it is; keep it.
3. UI: the collapsed sidebar shows favorites with a different indicator than the expanded sidebar. The human likes the collapsed one: use the same concept for favorites in the expanded sidebar, so both views read the same.

Not answered, so they stay as known non-blocking leftovers: the stale "command dropped" banner, no auto-scroll while dragging, `sync_favorite_order` after a later split, unescaped ids in `diag::warn`.
