VERDICT: PASS

## Blocking

## Notes
- The modal in `src/ui/reset_prompt.rs` follows the switcher and help window: frame fill `raised(0.06)`, 1px `raised(0.2)` stroke, radius 10, 15pt title font. All colors come from `UiColors` (`fg`, `bg`, `danger`, `raised`, `readable_on`) with no `Color32` literals. The Reset label uses `readable_on(danger)`, so contrast holds on light and dark themes.
- Sizes are in egui points (380 max width, 72 min button width), so they scale with UI zoom.
- The modal only draws while it is open, and nothing calls `request_repaint`, so idle CPU is unaffected. It has no animation of its own, so `reduce_motion` has nothing to gate.
- Enter, Esc and a click on the backdrop answer the prompt, and `overlay_open` keeps the terminal from taking those keys. The `initialized` guard stops the press that opened the prompt from answering it. The prompt is dropped if its tab closes or resets.
- The tab context menu entry "Reset" is shown for terminal tabs only, sits next to Duplicate, and shows no shortcut text because the action is unbound. The help window lists it under Tabs. Collapsed sidebar and zen mode are unaffected, since the prompt is app-level.
- Non-blocking: `Button::fill(...)` overrides egui's hover fill, so the two buttons get only the default hover expansion and stroke. A faint hover cue, such as lightening the fill from `resp.hovered()`, would match the help chips. Reset also doesn't show that Enter is its default; the danger fill reads as the primary action, which is probably enough.
- Non-blocking: if the process name is long, it wraps within the 380pt width, which is fine. The title is the tab title and could also be long, and it wraps too.
