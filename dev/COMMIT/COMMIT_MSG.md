fix: avoid modal spam on package-details fetch failures

- fix: skip Alert modal for official/AUR package-details-unavailable errors; only log.
- fix: show modal only for other network errors so UX stays clear.
- docs: update PR to document package-details error handling in event_loop.
