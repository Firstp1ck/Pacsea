# git-staged-msg

Create or refresh a **commit message** for **currently staged files only**.

## Scope and source of truth

- Use only `git diff --cached` / `git status --short` to determine content.
- Ignore unstaged, untracked, and previously committed changes.
- If there are no staged changes, stop and report that no message should be generated.

## Output files

Write to **`dev/COMMIT/`**:

- `dev/COMMIT/short.txt` -> subject line only
- `dev/COMMIT/message.txt` -> full commit message (subject + bullets)

If different filenames already exist in `dev/COMMIT/`, keep that existing convention, but still produce:

- one subject-only file
- one full-message file

## Critical overwrite rule

- Always **replace** file contents completely; never append.
- `message.txt` must contain exactly one commit message block.
- Do not leave older commit text below the new message.

## Conventional commits

Infer the lead type from the change set:

- `fix:`, `feat:`, `change:`, `perf:`, `test:`, `chore:`, `refactor:`, `docs:`, `style:`, `build:`, `ci:`, `revert:`

## Format

```
<type>: <short summary>

- <type>: bullet, short and specific
- <type>: bullet
```

## Content quality rules

- No prose outside the commit text block.
- Bullets must map to staged logical units.
- Keep bullets short, specific, and action-oriented.
- Use imperative style (`add`, `refactor`, `fix`) and avoid vague wording.
