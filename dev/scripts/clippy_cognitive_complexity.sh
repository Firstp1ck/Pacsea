#!/usr/bin/env bash
#
# Clippy Cognitive Complexity Report Generator
#
# This script runs cargo clippy and extracts cognitive complexity warnings/errors,
# then sorts them by complexity and provides an overview of files and functions
# that need adjustments.
#
# Usage:
#   ./dev/scripts/clippy_cognitive_complexity.sh
#
# Output:
#   - Displays sorted cognitive complexity issues
#   - Shows summary statistics
#   - Lists files and functions that exceed the threshold
#   - Saves full report to: dev/scripts/clippy_cognitive_complexity_report.txt

set -euo pipefail

# Colors for output (harmonized with Makefile)
COLOR_RESET=$(tput sgr0)
# shellcheck disable=SC2034  # Used in printf statements
COLOR_BOLD=$(tput bold)
COLOR_GREEN=$(tput setaf 2)
# shellcheck disable=SC2034  # Used in printf statements
COLOR_YELLOW=$(tput setaf 3)
COLOR_BLUE=$(tput setaf 4)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_ROOT"

REPORT_FILE="$SCRIPT_DIR/clippy_cognitive_complexity_report.txt"

printf "%bRunning cargo clippy to check for cognitive complexity issues...%b\n" "$COLOR_BLUE" "$COLOR_RESET"
echo ""

# Run clippy and capture output
CLIPPY_OUTPUT=$(cargo clippy --all-targets --all-features -- -D warnings 2>&1 || true)

# Check if clippy found any cognitive complexity issues
if ! echo "$CLIPPY_OUTPUT" | grep -q "cognitive complexity"; then
    printf "%b✓ No cognitive complexity issues found!%b\n" "$COLOR_GREEN" "$COLOR_RESET" | tee "$REPORT_FILE"
    exit 0
fi

# Extract cognitive complexity warnings/errors
# Pattern: "the function has a cognitive complexity of (XX/YY)"
# We'll extract: complexity value, file path, line number, function name

# Parse and extract information, output to both terminal and file
{
echo "=== Cognitive Complexity Issues ==="
echo ""

echo "$CLIPPY_OUTPUT" | awk '
BEGIN {
    in_error = 0
    complexity = 0
    threshold = 0
    file_path = ""
    line_num = ""
    function_name = ""
    issue_count = 0
}

# Match cognitive complexity error/warning line
/the function has a cognitive complexity of \([0-9]+\/[0-9]+\)/ {
    match($0, /\(([0-9]+)\/([0-9]+)\)/, arr)
    complexity = arr[1]
    threshold = arr[2]
    in_error = 1
    next
}

# Match file path and line number
in_error && /^[ ]+--> / {
    # Extract file path and line number
    # Format: "   --> src/path/to/file.rs:123:8" or "   --> src\path\to\file.rs:123:8" (Windows)
    if (match($0, /--> (.+):([0-9]+):/, arr)) {
        file_path = arr[1]
        line_num = arr[2]
    }
    next
}

# Match function name - appears on line with | and fn keyword
in_error && /\|.*fn / {
    # Extract function name
    # Format: " 17 | pub fn function_name(" or "149 | pub fn initialize_app_state("
    # Also handles: "149 | pub(crate) fn function_name("
    # Simple pattern: find "fn " followed by function name
    if (match($0, /fn ([a-zA-Z0-9_]+)/, fn_arr)) {
        function_name = fn_arr[1]
    }
    next
}

# Match function name from the line with the ^ marker (fallback)
in_error && /^\^/ && function_name == "" {
    # Sometimes the function name is on the previous line, try to extract from context
    next
}

# Helper function to save current issue
function save_issue() {
    if (complexity > 0 && file_path != "") {
        issues[++issue_count] = complexity "|" threshold "|" file_path "|" line_num "|" function_name
        complexity = 0
        threshold = 0
        file_path = ""
        line_num = ""
        function_name = ""
    }
    in_error = 0
}

# When we hit a blank line, save the previous one
in_error && /^$/ {
    save_issue()
    next
}

# Also save when we see "error: could not compile" as that indicates end of errors
in_error && /^error: could not compile/ {
    save_issue()
    next
}

END {
    # Save last issue if any
    if (complexity > 0 && file_path != "") {
        issues[++issue_count] = complexity "|" threshold "|" file_path "|" line_num "|" function_name
    }
    
    if (issue_count == 0) {
        print "No cognitive complexity issues found."
        exit 0
    }
    
    # Sort by complexity (descending)
    n = asort(issues, sorted_issues)
    
    # Print header
    printf "%-6s %-6s %-50s %-6s %s\n", "COMPLEX", "THRESH", "FILE", "LINE", "FUNCTION"
    printf "%s\n", "------------------------------------------------------------------------------------------------------------------------"
    
    # Print sorted issues
    for (i = n; i >= 1; i--) {
        split(sorted_issues[i], parts, "|")
        comp = parts[1]
        thresh = parts[2]
        file = parts[3]
        line = parts[4]
        func_name = parts[5]
        
        # Truncate long file paths for display
        if (length(file) > 48) {
            file = "..." substr(file, length(file) - 45)
        }
        
        printf "%-6s %-6s %-50s %-6s %s\n", comp, thresh, file, line, func_name
    }
    
    print ""
    print "=== Summary ==="
    print ""
    print "Total issues: " issue_count
    
    # Count by file
    delete file_counts
    delete file_max_comp
    for (i = n; i >= 1; i--) {
        split(sorted_issues[i], parts, "|")
        file = parts[3]
        comp = parts[1] + 0
        
        file_counts[file]++
        if (file_max_comp[file] == "" || comp > file_max_comp[file]) {
            file_max_comp[file] = comp
        }
    }
    
    print ""
    print "Files needing attention (" length(file_counts) " files):"
    print ""
    
    # Sort files by max complexity
    file_idx = 0
    for (file in file_counts) {
        file_list[++file_idx] = file_max_comp[file] "|" file_counts[file] "|" file
    }
    n_files = asort(file_list, sorted_files)
    
    printf "%-6s %-6s %s\n", "MAX", "COUNT", "FILE"
    printf "%s\n", "--------------------------------------------------------"
    for (i = n_files; i >= 1; i--) {
        split(sorted_files[i], parts, "|")
        max_comp = parts[1]
        count = parts[2]
        file = parts[3]
        printf "%-6s %-6s %s\n", max_comp, count, file
    }
    
    print ""
    print "=== Recommendations ==="
    print ""
    print "1. Functions with complexity > 50: Consider significant refactoring"
    print "2. Functions with complexity 30-50: Should be split into smaller functions"
    print "3. Functions with complexity 25-30: Minor refactoring recommended"
    print ""
    print "You can suppress warnings for existing code using:"
    print "  #[allow(clippy::cognitive_complexity)]"
    print ""
    print "Or adjust the threshold in clippy.toml:"
    print "  cognitive-complexity-threshold = <value>"
}
'

echo ""
echo "=== Full Clippy Output ==="
echo ""
echo "$CLIPPY_OUTPUT"
} | tee "$REPORT_FILE"

echo ""
printf "%bReport saved to: %s%b\n" "$COLOR_BLUE" "$REPORT_FILE" "$COLOR_RESET"

