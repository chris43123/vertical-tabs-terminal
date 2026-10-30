VERDICT: PASS

## Blocking

## Notes
- Selection color comes from `UiColors.selection` (theme selection, falling back to a bg/fg mix when it is invisible). No hardcoded `Color32` in chrome code; the `from_rgb` calls in code.rs are in tests only.
- Min-contrast is honored for text over the selection (`min_contrast > 1.0` re-lays out the galley).
- The Copy button is a `small_button` text label, matching the neighboring Wrap and Source toolbar items. Its hover text changes with the selection state.
- The pointer shows the text cursor over the body and the default cursor over the gutter.
- Auto-scroll repaints only while the offset actually changes, so idle CPU stays at 0. It is a drag-driven scroll rather than an animation, so `reduce_motion` does not apply.
- Sizes: the new gutter and selection constants (MARGIN, PAD_Y, 16.0 padding, 0.6 × char_width) are font-relative or match the existing values, so they scale with zoom.
- Non-blocking: the 24 px auto-scroll edge is fixed in points. egui scales points with zoom, so this is fine.
