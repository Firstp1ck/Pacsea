#!/usr/bin/env bash

# Script to analyze Clippy errors and output formatted statistics

# Colors for output (harmonized with Makefile)
COLOR_RESET=$(tput sgr0)
COLOR_BOLD=$(tput bold)
# shellcheck disable=SC2034  # Used in printf statements
COLOR_GREEN=$(tput setaf 2)
# shellcheck disable=SC2034  # Used in printf statements
COLOR_YELLOW=$(tput setaf 3)
COLOR_BLUE=$(tput setaf 4)

# Always output clippy_errors.txt in the same directory as this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_FILE="${SCRIPT_DIR}/clippy_errors.txt"
TEMP_FILE=$(mktemp)

printf "%bRunning cargo clippy (this may take a moment)...%b\n" "$COLOR_BLUE" "$COLOR_RESET" >&2

# Run clippy and extract all error messages
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | \
    grep "^error:" | \
    sed 's/^error: //' > "$TEMP_FILE"

# Process clippy lints (exclude compilation errors)
CLIPPY_LINTS=$(grep -v "could not compile" "$TEMP_FILE" | \
    sort | uniq -c | sort -rn)

# Process compilation errors separately
COMPILE_ERRORS=$(grep "could not compile" "$TEMP_FILE" | \
    sort | uniq -c | sort -rn)

# Output clippy lints
printf "%b=== Clippy Lints ===%b\n" "$COLOR_BOLD" "$COLOR_RESET" | tee "$OUTPUT_FILE"
if [ -n "$CLIPPY_LINTS" ]; then
    echo "$CLIPPY_LINTS" | \
        awk '{
            count = $1
            total += count
            $1 = ""
            error_msg = substr($0, 2)  # Remove leading space
            printf "%5d %s\n", count, error_msg
        } END {
            printf "\n%5d total clippy lints\n", total
            printf "%5d unique lint types\n", NR
        }' | tee -a "$OUTPUT_FILE"
else
    echo "No clippy lints found." | tee -a "$OUTPUT_FILE"
fi

# Output compilation errors separately
echo "" | tee -a "$OUTPUT_FILE"
printf "%b=== Compilation Errors ===%b\n" "$COLOR_BOLD" "$COLOR_RESET" | tee -a "$OUTPUT_FILE"
if [ -n "$COMPILE_ERRORS" ]; then
    echo "$COMPILE_ERRORS" | \
        awk '{
            count = $1
            total += count
            $1 = ""
            error_msg = substr($0, 2)  # Remove leading space
            printf "%5d %s\n", count, error_msg
        } END {
            printf "\n%5d total compilation errors\n", total
        }' | tee -a "$OUTPUT_FILE"
else
    echo "No compilation errors found." | tee -a "$OUTPUT_FILE"
fi

# Cleanup
rm -f "$TEMP_FILE"

echo "" >&2
printf "%bResults saved to: %s%b\n" "$COLOR_BLUE" "$OUTPUT_FILE" "$COLOR_RESET" >&2

