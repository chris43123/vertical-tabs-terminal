VERDICT: PASS

## Blocking

## Notes
- Round 1 notes unchanged. The new copy toast is non-interactive, takes no keys or focus, has no new config key, and honors reduce_motion. Terminal copies get no toast, so terminal behavior is unchanged.
- A failed clipboard write is shown in the toast (danger color) rather than the config banner. That is fine, since it is not a config error.
- Ctrl+A and Esc are only taken in a focused preview source view (Esc only when a selection exists), so the shell and terminal programs keep them.
- README describes the toast. Manual checks 5-9 are still not run.
