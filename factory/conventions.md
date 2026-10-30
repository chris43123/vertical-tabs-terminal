# vtt conventions (every factory agent reads this)

vtt is a lightweight, GPU-accelerated terminal with a vertical tab sidebar, written in
Rust (edition 2024) with egui on wgpu and alacritty_terminal. It runs on Linux
(Wayland/X11), Windows (ConPTY) and macOS. See the "Why vtt" section of README.md.

## Checks (must all pass, same as CI)

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

In a job worktree, share the main build cache so you don't rebuild from scratch:
prefix cargo with `CARGO_TARGET_DIR=<repo root>/target`.

## Layout

The crate layout is documented at the top of `src/main.rs`:
`config` → `theme` → `terminal` → `workspace` → `files`/`preview` → `render` → `app` → `ui`.
`workspace` is pure bookkeeping (no rendering, no sessions) and is where most unit tests live.

## Code

- Tests are inline: `#[cfg(test)] mod tests` at the bottom of the file under test.
- Comments are sparse and explain why. `//!` module docs, `///` on public items.
- No new dependency unless it is pure Rust, modest, and clearly earns its place.
- Platform code goes behind `cfg(unix)` / `cfg(windows)` / `cfg(target_os = "macos")`;
  it must still compile and pass clippy on all three.

## Product rules

- **Slim on purpose.** Idle CPU stays ~0%: redraw only when something changes, no
  polling loops or constant repaints. Event-driven designs win.
- **A terminal, not an editor.** vtt shows files (tree, previews, git status); it never
  edits them. Where a good CLI exists (`git`), run it rather than reimplement it.
- **No invented keybinds.** On Linux/Windows a new action gets a default only if it is an
  established convention for that exact action (terminal / VS Code / browser norms).
  Never add `Alt+<key>` defaults; Alt belongs to the shell. Otherwise ship it unbound,
  in the command palette, with an example in `config.example.toml`. macOS uses ⌘.
- **Theme-driven colors.** UI colors come from `theme::chrome::UiColors` (derived from the
  terminal theme, light or dark). No hardcoded `Color32` values in `src/ui`.
- **Accessibility settings are honored**: UI zoom, `reduce_motion`, minimum contrast.

## Docs

A user-visible change updates README.md (Features, the keybinding tables, Configuration).
A new config key goes in both `config.example.toml` and the copy of it in README.md.

## Commits

Subject: imperative, sentence case, no prefix, no trailing period
("Add an app icon", "Give shells a UTF-8 locale when started from Finder").
Body wrapped at ~72 columns, explaining what and why. End with:

    Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>

Never commit to `main` directly; the orchestrator merges.

## Push everything

Every change goes to GitHub as soon as its checks pass: work happens on more than one
machine, and anything left only on a local disk is lost to the others.

- Builder: after each commit that passes the checks, `git push -u origin factory/<id>`.
- Orchestrator: `factory/merge.sh` pushes `main` after merging and removes the remote
  job branch.
- Never push a commit whose checks fail. Never force-push.
