#!/bin/bash

# Script to analyze Clippy errors and output formatted statistics

# Always output clippy_errors.txt in the same directory as this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_FILE="${SCRIPT_DIR}/clippy_errors.txt"

echo "Running cargo clippy (this may take a moment)..." >&2

# Run clippy and extract error messages, save to both stdout and file
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | \
    grep "^error:" | \
    sed 's/^error: //' | \
    # Count occurrences and sort by count (descending)
    sort | uniq -c | sort -rn | \
    # Format output: right-align count, then full error message, and accumulate total
    awk '{
        count = $1
        total += count
        $1 = ""
        error_msg = substr($0, 2)  # Remove leading space
        printf "%5d %s\n", count, error_msg
    } END {
        printf "\n%5d total errors\n", total
    }' | tee "$OUTPUT_FILE"

echo "" >&2
echo "Results saved to: $OUTPUT_FILE" >&2

