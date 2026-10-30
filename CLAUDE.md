# vtt

Repo rules live in `factory/conventions.md`; read it before changing code.

If `factory/HANDOFF.md` exists, read it first: it is the state left by the last
session on another machine. Its "Cierre" section lists what must be true for it to go.
Check it at the start and after each step; once every point holds, delete the file
in its own commit and push. Don't leave a finished handoff behind.

## Push everything

This repo is worked on from more than one machine. Anything that exists only on a
local disk is lost to the others, so:

- **Start of a session:** `git fetch origin` and bring the current branch up to date
  (`git pull --ff-only`) before changing anything. Say so if it diverged.
- **Every change, once tested, is committed and pushed to GitHub.** Tested means the
  checks in `factory/conventions.md` pass (`cargo fmt --check`, `clippy -D warnings`,
  `cargo test`); docs- or config-only changes need no cargo run. This is standing
  permission to push; don't ask first.
- Work on a branch, never commit straight to `main` outside `factory/merge.sh`.
  Push with `git push -u origin <branch>`. If the change is finished, open a PR.
- Never push failing code, never force-push.
- Factory jobs follow the same rule; see "Push everything" in `factory/conventions.md`.
