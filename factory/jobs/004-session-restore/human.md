## 2026-10-30T21:30:59Z

Manual checks are all OK. Keep `restore_session` on by default.

Change: when a restored tab starts (first focus), only open its shell in the last known folder. Do not type the saved command again, so no long-running processes start on return. This applies to every restored tab, not just ones with a substituted profile.
