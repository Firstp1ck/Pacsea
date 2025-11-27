# Rust Development Rules

## When Creating New Code (Files, Functions, Methods, Enums)
- Always check cyclomatic complexity < 25
- Always check data flow complexity < 25
- Always add rust docs to classes/methods/functions (What, Inputs, Output, Details)
- Always add logical important unit tests
- Always add logical important integration tests

## When Fixing Bugs/Issues
- Check deeply what the issue is
- Create tests that fail for the specific issue
- Run the created tests: it should fail, if not adjust the test
- Solve the issue
- Check if test passes, if not continue trying other solutions

## Always Run After Changes
- cargo fmt
- cargo clippy --all-targets --all-features -- -D warnings
- cargo check
- cargo test -- --test-threads=1

## Cargo Clippy Configuration
Check with cargo clippy after adding a new feature and fix clippy errors with the following settings:
```toml
[lints.clippy]
# Enable cognitive complexity lint to catch overly complex functions
cognitive_complexity = "warn"
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }
unwrap_used = "deny"
```

## General Rules
- Do not create *.md files, unless explicitly asked
