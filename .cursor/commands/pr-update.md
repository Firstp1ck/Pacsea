# pr-update

compare current branch against `main` (`git log main..HEAD` + `git diff --name-status main...HEAD`) and update the matching file in `dev/PR/` for the active branch.

integrate missing updates into existing sections (`Summary`, `Related issues`, `How to test`, `Files changed`, checklist) so the PR reads as one coherent document. do not use a standalone "additional updates" append-only section unless explicitly requested.

when updating:
- keep wording short, specific, and reviewer-focused
- include only changes that differ from `main` (final branch state)
- keep existing valid entries, but rewrite/merge bullets when needed for clarity and deduplication
- remove stale statements that no longer match branch reality (for example "remaining work" that is now complete)
- ensure `How to test` reflects current test coverage for the implemented scope
