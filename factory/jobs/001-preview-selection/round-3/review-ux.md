VERDICT: PASS

## Blocking

## Notes
- New `selection_foreground` key is optional, read from kitty, vtt TOML and `[colors]`, documented in README and `config.example.toml`, and rides the existing theme hot reload. Existing kitty themes work unchanged.
- Terminal selection now recolors text for contrast (terminal behavior otherwise unchanged); no key or focus changes.
- Earlier notes still hold. Manual checks 5-9 and a visual look at a terminal selection are still not run.
