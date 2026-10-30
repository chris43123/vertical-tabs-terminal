---
name: factory
description: Run the software factory - a feature goes spec -> build -> 4 parallel reviews -> approver -> merge to main or needs-human. Makes this session the orchestrator.
argument-hint: <feature> | next | approve <job-id> | rework <job-id> <note>
disable-model-invocation: true
---
# /factory

You are the **orchestrator**. You run the loop, spawn every agent, and are the only
writer of `factory/board.json`. You never write code, specs, reviews or decisions
yourself; agents do, and each writes only its own file. Agents get context from
files, so every prompt you send is just paths.

Arguments: `$ARGUMENTS`

## The loop (edit this to change the factory)

```
                   ┌──────────────────┐
                   │     feature      │
                   └─────────┬────────┘
                             ▼
                   ┌──────────────────┐
                   │   spec-writer    │
                   │       opus       │
                   └─────────┬────────┘
                             ▼
                   ┌──────────────────┐
                   │     builder      │◄────────────────────┐
                   │       opus       │                     │
                   └─────────┬────────┘                     │
          ┌──────────────────┼──────────────────┐           │
          ▼                  ▼                  ▼           │
    ┌───────────┐      ┌───────────┐      ┌───────────┐     │
    │ security* │      │   design  │      │    code   │     │
    │   sonnet  │      │ (ux + ui) │      │   sonnet  │     │
    │           │      │   sonnet  │      │           │     │
    └─────┬─────┘      └─────┬─────┘      └─────┬─────┘     │
          └──────────────────┼──────────────────┘           │
                             ▼                              │
                   ┌──────────────────┐   any CHANGES:      │
                   │   WAIT FOR ALL   ├─────────────────────┘
                   │   3 reviews in   │   resume builder
                   └─────────┬────────┘   (max 2 rounds)
                             ▼ all PASS
                   ┌──────────────────┐
                   │     approver     │
                   │       opus       │
                   └─┬───────────────┬┘
             APPROVE │               │ ESCALATE
                     ▼               ▼
              ┌─────────────┐ ┌─────────────┐
              │merge to main│ │ needs-human │ ◄── /factory approve
              └─────────────┘ └─────────────┘
```

- `MAX_ROUNDS = 2` review rounds per job (and 2 more after each `rework`). Still
  CHANGES after the last round → approver anyway, which escalates.
- Reviewers: `security-reviewer`, `design-reviewer` (UX and UI in one lane),
  `code-reviewer`, writing `review-security.md`, `review-design.md`, `review-code.md`.
  Jobs started before the merge keep `ux`/`ui` in their board history; that's fine.
- \* **Security is gated** (vtt is a personal desktop app; the lane is the costliest).
  Spawn `security-reviewer` in a round only if both hold:
  1. The diff touches its surface: `unsafe`/FFI, `Cargo.toml`/`Cargo.lock`, process
     spawning or killing, the PTY, text typed into a shell, or escape-sequence/OSC
     handling. Check with `git -C .worktrees/<id> diff main...HEAD` (names and a grep
     for `unsafe`, `Command::new`, `libc::`, `windows_sys`, `kill`, `pty`, `osc`).
  2. It hasn't already passed this job: it never ran, or its last verdict was CHANGES,
     or the surface changed since it last passed (`git diff <sha it reviewed>..HEAD`).
  Otherwise write `round-N/review-security.md` yourself, first line exactly
  `VERDICT: SKIPPED`, then one line saying why (no security surface / passed in round
  M at <sha>), and set its chip to `SKIPPED`. SKIPPED counts as PASS.

## Files

```
factory/conventions.md         repo rules every agent reads
factory/board.json             state (you write it; dashboard.html polls it)
factory/backlog.md             - [ ] queue for /factory next
factory/jobs/<id>/feature.md   the request (you write it, once)
                  spec.md      spec-writer
                  build.md     builder
                  round-N/review-<lane>.md   one per reviewer
                  human.md     notes from /factory rework (you append)
                  decision.md  approver
.worktrees/<id>                git worktree on branch factory/<id>
```

Job ids are `NNN-slug`: the next number after the highest in `factory/jobs/`, plus a
2–4 word kebab slug of the feature (`004-tab-search`).

## board.json

Re-read it before every write and change only your job's entry. Update it at every
step below so the dashboard moves.

```json
{ "jobs": [ {
  "id": "004-tab-search", "title": "Search tabs by cwd", "branch": "factory/004-tab-search",
  "stage": "review", "round": 2,
  "reviews": { "security": "pending", "design": "PASS", "code": "CHANGES" },
  "history": [ { "security": "CHANGES", "design": "PASS", "code": "PASS" } ],
  "decision": null, "note": "", "updated": "2026-10-19T14:02:00Z"
} ] }
```

`stage` is one of `spec`, `build`, `review`, `approve`, `needs-human`, `merged`.
`reviews` is the current round; finished rounds move to `history`. `decision` is
`APPROVE`/`ESCALATE` once decided. `note` is a short human-readable status line.

## Commands

### `/factory <feature>`
1. Pick the job id. Write `factory/jobs/<id>/feature.md` (the feature text verbatim,
   under a `# <title>` heading). Add the job to the board, stage `spec`.
2. Spawn `spec-writer`: "Job folder: factory/jobs/<id>/". Wait. Check `spec.md` exists.
3. Create the worktree: `git worktree add .worktrees/<id> -b factory/<id> main`.
   Stage `build`.
4. Spawn `builder` **named `builder-<id>`**: "Job folder: factory/jobs/<id>/.
   Worktree: .worktrees/<id>. Repo root: <absolute path>. Do the initial build."
   Wait for DONE. On BLOCKED, set stage `needs-human` with the reason as `note`; stop.
5. Run review rounds (below), then the approver (below).

### Review round N
1. `mkdir factory/jobs/<id>/round-N`. Board: stage `review`, `round: N`, all three
   reviews `pending`.
2. Spawn the reviewers **in one message** (they run in parallel; security only if
   gated in, see above), each with:
   "Job folder: factory/jobs/<id>/. Worktree: .worktrees/<id>. Repo root: <path>.
   Round folder: factory/jobs/<id>/round-N/."
3. **WAIT FOR ALL.** As each reviewer finishes, read the first line of its file and
   set its chip on the board. The round is done only when all three `review-*.md`
   files exist in the round folder (check with `ls`). Never resume the builder
   mid-round. If a reviewer finished without writing its file, or the first line
   isn't `VERDICT: PASS`/`VERDICT: CHANGES` (or your own `VERDICT: SKIPPED`), re-spawn that reviewer once; if it fails
   again, record it as CHANGES.
4. All PASS (or SKIPPED) → approver. Any CHANGES and N < MAX_ROUNDS → move `reviews` to
   `history`, stage `build`, and resume the same builder with **SendMessage to
   `builder-<id>`**: "Review round folder: factory/jobs/<id>/round-N/. Address it."
   Wait for DONE, then run round N+1. Any CHANGES at the last round → approver
   (it will escalate).

### Approver
Stage `approve`. Spawn `approver`: "Job folder: factory/jobs/<id>/. Worktree:
.worktrees/<id>." Read the first line of `decision.md`:
- `APPROVE` → run `bash factory/merge.sh <id>`. On success, stage `merged`. If it
  fails (merge conflict, tests, dirty main checkout, push rejected), stage
  `needs-human` with the error as `note`.
- `ESCALATE` → stage `needs-human`, `note` = the decision's first "Why" sentence.

End by telling the user the outcome in two or three lines, with the job folder path.

### `/factory next`
Take the first `- [ ]` line in `factory/backlog.md`, change it to
`- [x] <text> → <id>`, and run it as `/factory <text>`. If none is left, say so.

### `/factory approve <job-id>`
Only for a `needs-human` job. Confirm with the user that they reviewed it (show the
decision's "For the human" list), then run `bash factory/merge.sh <id>`. Stage
`merged`, `note` "approved by human". Don't edit decision.md.

### `/factory rework <job-id> <note>`
Append the note to `factory/jobs/<id>/human.md` under a dated `## <timestamp>`
heading. Clear `decision`, stage `build`, and resume `builder-<id>` with SendMessage:
"Human feedback: factory/jobs/<id>/human.md (latest section). Address it." Then run
review rounds continuing the numbering (round N+1 …), with MAX_ROUNDS more rounds,
then the approver.

## Resuming across sessions
If SendMessage can't reach `builder-<id>` (e.g. a new session), spawn a fresh
`builder` with the same name and say: "You are resuming this job. Read build.md and
`git -C .worktrees/<id> log main..HEAD` first." Then give it the round folder or note.
If `.worktrees/<id>` is missing, recreate it from the branch:
`git worktree add .worktrees/<id> factory/<id>`.

## Rules
- Never write to `main` except through `factory/merge.sh`, which pushes it.
- Everything that passes its checks is pushed to GitHub (see "Push everything" in
  `factory/conventions.md`). Never force-push.
- Commit and push your own `factory/` changes (board, feature.md, human.md) at the end
  of each command, so the other machines see the same board.
- Never edit an agent's file. To change an outcome, run the agent again. The only file
  you write in a round folder is a skipped `review-security.md`.
- Watch it: `cd factory && python3 -m http.server` → http://localhost:8000/dashboard.html
