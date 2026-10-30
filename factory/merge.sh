#!/usr/bin/env bash
# Merge a factory job into main: bring main (and GitHub's main) into the branch, re-test,
# fast-forward main, push it.
# Usage: factory/merge.sh <job-id>
set -euo pipefail
id=$1
root=$(git rev-parse --show-toplevel)
wt="$root/.worktrees/$id"
branch="factory/$id"

# main may be checked out in some worktree; move it there, else move the ref.
main_wt=$(git worktree list --porcelain | awk '/^worktree /{w=substr($0,10)} $0=="branch refs/heads/main"{print w}')
ff_main() {
  git merge-base --is-ancestor "$1" main && return 0
  if [ -n "$main_wt" ]; then
    git -C "$main_wt" merge --ff-only "$1"
  else
    git merge-base --is-ancestor main "$1" || { echo "main has diverged from $1" >&2; return 1; }
    git branch -f main "$1"
  fi
}

# Pick up anything pushed from another machine first.
git fetch origin
ff_main origin/main

git -C "$wt" merge --no-edit main
(cd "$wt" && CARGO_TARGET_DIR="$root/target" cargo test --quiet)

ff_main "$branch"
git push origin main

git worktree remove "$wt"
git merge-base --is-ancestor "$branch" main && git branch -D "$branch"
git push origin --delete "$branch" 2>/dev/null || true
echo "merged $branch into main and pushed ($(git rev-parse --short main))"
