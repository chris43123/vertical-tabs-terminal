# Backlog

`/factory next` runs the first unchecked item and checks it off (`→ <job-id>`).
One feature per line; a sentence or two is plenty, the spec-writer fills in the rest.

- [x] Launch recipes + "Reset tab": each tab remembers how it was launched; reset kills its process tree and respawns it. Job 1 of factory/plans/sessions-and-favorites.md; follow that plan, including its CLI requirement. → 002-launch-recipes-reset
- [x] Favorite directories as pinned sessions in the sidebar, with `vtt favorites` CLI. Job 2 of factory/plans/sessions-and-favorites.md; needs job 1 merged. → 003-favorite-sessions
- [ ] Restore open tabs on launch (dormant until focused), with `vtt session` CLI. Job 3 of factory/plans/sessions-and-favorites.md; needs job 1 merged.
- [ ] Show a tab's exit status in its hover card when the process has exited (code, or the signal that killed it).
- [ ] "Copy working directory" action in the command palette for the focused tab (unbound by default).
- [ ] Files panel: show untracked files in a distinct color from modified ones, using `git status` output vtt already reads.
