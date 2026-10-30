VERDICT: CHANGES

## Blocking
1. src/favorites/mod.rs:86-113 (`Favorites::parse`) and src/app/favorites.rs:173-184 (`favorite_recipe`)
   — the single-line command rule is only checked on the CLI and dialog write paths. The
   file itself is documented as hand-editable (the header written by `to_toml` says "or by
   hand"), and TOML basic strings take `\n`, `\r` and `\u001b` escapes. So
   `command = "make\nrm -rf ~"` or a command with an ESC in it loads fine, and
   `LaunchRecipe::typed_input` sends it to the shell as is. `trim_end_matches` only strips
   trailing CR/LF, so this types two commands, or a command plus raw escape bytes. That
   breaks the spec's promise that "`typed_input` only ever sends one Enter", which the
   job 002 decision asked for. Names have the same gap: `validate` rejects control
   characters, but a hand-edited name with ESC loads, and `vtt favorites list` then prints
   it raw to the user's terminal (escape injection through the CLI's output). The fix is
   to enforce the rules on load as well. In `parse` (or a load-time pass in
   `Store::load` / `FavoritesState::replace`), run `check_command` on every `command` and
   the control-character check on every `name`. A favorite that fails should be dropped
   from what gets typed (for example, set its command to `None` and mark the row), with a
   `diag::warn` naming the favorite, the same way unknown profiles are handled. Don't fail
   the whole file, because that would hide the other favorites. As a second guard,
   `favorite_recipe` (used for both open and Reset) can refuse a command that fails
   `check_command`. Add a unit test: a file with a `\n` / `\u001b` command loads, but the
   command is never typed.

## Notes
- Checked and fine: no `unsafe`, no FFI, no new dependencies (Cargo.toml is unchanged).
  Temp-file-then-rename write in the same directory, cleaned up on failure. The id and
  index arithmetic can't panic (`move_favorite` only clamps after the id was found, so the
  list isn't empty). `first_grapheme` goes by chars, not byte slices. Bad TOML is a
  warning that keeps the previous list, never a panic.
- `to_toml` uses `expect("favorites serialize")`. This can't fail for these types today
  (strings and paths; a non-UTF-8 `PathBuf` would fail, but every stored path comes from
  `canonicalize` of a `&str`, or from a TOML string). It's acceptable, but returning a
  `Result` would remove the last panic path if paths ever come from elsewhere.
- `list_text` also prints `path` and `profile` raw. Once names and commands are checked
  on load, consider escaping control characters in `list` output too (for example with
  `char::escape_default` on any control character), since the path comes from the file.
- The temp name `.favorites.toml.<pid>.tmp` is predictable, but it lives in the user's own
  data directory, so it isn't a cross-user concern.
- The spec's trust model holds: anything that can write `favorites.toml` can run a
  command when the user clicks. That's rc-file level, and the README's Favorites section
  could say so in one line.
