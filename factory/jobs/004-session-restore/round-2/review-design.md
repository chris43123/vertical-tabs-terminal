VERDICT: PASS
Reviewed: d724dd7

## Blocking

## Notes
- Round 1 point 1 is fixed. The dormant title now uses `ensure_contrast(mix(text, sidebar, 0.55), sidebar, DIM_CONTRAST.max(min_contrast))`, the same dim as a missing favorite, and `DIM_CONTRAST` is one shared constant in `ui/sidebar/mod.rs`. The color comes from the theme and the contrast setting, so light and dark themes are covered.
- Only that color changed since the last review, so nothing else in my lane was re-reviewed.
- For the manual pass: check by eye that a dormant row is still clearly different from an active one on a light theme, and that the 0.55 dim still reads as "not started" when the floor lifts it close to normal text.
