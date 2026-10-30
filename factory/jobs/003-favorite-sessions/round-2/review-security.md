VERDICT: PASS

## Blocking

## Notes
- Round 1 #1 is fixed in ac094cb. `Favorites::parse` now runs `sanitize` on every
  entry. A command that fails `check_command` is set to `None`, control characters in a
  name become U+FFFD, and each fix is reported as a problem without failing the rest of
  the file. `favorite_recipe` filters the command through `check_command` a second time,
  so open and Reset can only type one plain line, whatever the source. The tests
  `hand_edited_control_characters_never_load`,
  `a_hand_edited_multiline_command_is_never_typed` and
  `list_escapes_control_characters_from_the_file` cover it. `cargo test favorites`
  passes (28 tests).
- The round 1 note on `list` output is done. `printable` escapes control characters in
  id, name, path, profile and command, and in the problem warnings the CLI prints.
- Leftover, not blocking: in the app, `warn_problems` and the unknown-profile warning go
  through `diag::warn`, which `eprintln!`s the message without escaping. The name is
  already sanitized, but `f.id` (and an unknown profile name) from a hand-edited file
  can still carry ESC to stderr. That only matters when vtt was started from another
  terminal, and the writer already has rc-file trust. Consider escaping the id in
  `sanitize`'s message, or sanitizing ids on load too.
- `is_control` covers only Cc. Bidi or format characters (U+202E and the like) in names
  pass. In egui that only affects how the text looks; worth a later look if names get
  shown in the CLI next to other columns.
- The next mutation re-reads and sanitizes the file, then writes it back, so a
  hand-edited bad command is dropped from the file for good. That is safe, and the
  warning tells the user. Mentioning it in the README would avoid surprise.
- Unchanged: no `unsafe`, no FFI, no new dependencies. `to_toml`'s `expect` is still
  acceptable for these types.
