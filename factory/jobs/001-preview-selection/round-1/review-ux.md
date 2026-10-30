VERDICT: PASS

## Blocking

## Notes
- Keys are scoped to a focused preview tab (preview_key only fires for source views; terminal path unchanged). No Alt chords, no new config key or Action. Ctrl+A/Ctrl+C match the spec risk rationale.
- Rendered markdown loses its selection once the anchor scrolls off screen (build.md item 6). The README says so and points to Source plus line numbers, so it is not blocking. It still leaves a gap in the "drag past the edge" promise for markdown. The help line "near the edge it scrolls (markdown too)" could add that long selections should use Source.
- Copy reference is only in the right-click menu. Consider an unbound palette action later (spec open question).
- Manual checks (criteria 5-9, especially terminal Ctrl+C unchanged and idle CPU) were not run by the builder.
