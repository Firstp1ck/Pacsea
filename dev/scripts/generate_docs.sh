#!/bin/bash
#
# Rust Documentation Generator
#
# This script generates comprehensive Rust documentation for the Pacsea project using cargo doc.
# It creates HTML documentation with code examples, API references, and cross-references.
#
# What it does:
#   1. Generates HTML documentation for all Rust code in the project
#   2. Includes private items (--document-private-items) for complete API coverage
#   3. Excludes external dependencies (--no-deps) to focus on project code only
#   4. Displays a dependency tree showing the project's external dependencies
#
# Output:
#   - Documentation is generated in target/doc/
#   - Main entry point: target/doc/pacsea/index.html
#   - Can be viewed by running: cargo doc --open
#
# Features:
#   - Includes private/internal items for comprehensive documentation
#   - Cross-referenced links between modules and functions
#   - Syntax-highlighted code examples
#   - Search functionality
#   - Dependency tree visualization (depth 2 levels)
#
# Usage:
#   ./generate_docs.sh
#   cargo doc --open  # View the generated documentation
#
# Requirements:
#   - Rust toolchain (cargo)
#   - Project must compile successfully
#

set -e

# Colors for output (harmonized with Makefile)
COLOR_RESET=$(tput sgr0)
COLOR_BOLD=$(tput bold)
COLOR_GREEN=$(tput setaf 2)
COLOR_YELLOW=$(tput setaf 3)
COLOR_BLUE=$(tput setaf 4)

printf "%bGenerating Rust documentation...%b\n" "$COLOR_BLUE" "$COLOR_RESET"
cargo doc --no-deps --document-private-items

printf "%bDocumentation generated in target/doc/%b\n" "$COLOR_GREEN" "$COLOR_RESET"
printf "%bOpen with: cargo doc --open%b\n" "$COLOR_BLUE" "$COLOR_RESET"

# Optional: Generate dependency tree
echo ""
printf "%bDependency tree:%b\n" "$COLOR_BOLD" "$COLOR_RESET"
cargo tree --depth 2

