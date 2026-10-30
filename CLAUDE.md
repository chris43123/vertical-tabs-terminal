# vtt

A lightweight, GPU-accelerated terminal with a vertical tab sidebar, in Rust (edition
2024) with egui on wgpu and alacritty_terminal. Runs on Linux (Wayland/X11), Windows
(ConPTY) and macOS. See "Why vtt" in README.md.

## Push everything

This repo is worked on from more than one machine. Anything that exists only on a
local disk is lost to the others, so:

- **Start of a session:** `git fetch origin` and bring the current branch up to date
  (`git pull --ff-only`) before changing anything. Say so if it diverged.
- **Every change, once tested, is committed and pushed to GitHub.** Tested means the
  checks below pass; docs- or config-only changes need no cargo run. This is standing
  permission to push; don't ask first.
- Work on a branch, never commit straight to `main`. Push with
  `git push -u origin <branch>`. If the change is finished, open a PR.
- Never push failing code, never force-push.

## Checks (same as CI, must all pass)

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs clippy and tests on Linux, Windows and macOS.

## Layout

The module layout is documented at the top of `src/main.rs`. `workspace` is pure
bookkeeping (no rendering, no sessions) and is where most unit tests live.

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
- **Accessibility settings are honored**: UI zoom, `reduce_motion`, `min_contrast`.

## Docs

A user-visible change updates README.md (Features, the keybinding tables, Configuration).
A new config key goes in both `config.example.toml` and the copy of it in README.md.

## Commits

Subject: imperative, sentence case, no prefix, no trailing period
("Add an app icon", "Give shells a UTF-8 locale when started from Finder").
Body wrapped at ~72 columns, explaining what and why.
