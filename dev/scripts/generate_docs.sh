#!/bin/bash
# Generate Rust documentation with dependency graphs

set -e

echo "Generating Rust documentation..."
cargo doc --no-deps --document-private-items

echo "Documentation generated in target/doc/"
echo "Open with: cargo doc --open"

# Optional: Generate dependency tree
echo ""
echo "Dependency tree:"
cargo tree --depth 2

