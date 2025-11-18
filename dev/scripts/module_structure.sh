#!/bin/bash
# Generate module structure visualization using cargo-modules

set -e

# Check if cargo-modules is installed
if ! command -v cargo-modules &> /dev/null; then
    echo "Installing cargo-modules..."
    cargo install cargo-modules
fi

echo "Generating module tree (library)..."
cargo modules structure --lib > module_tree.txt
echo "Module tree saved to module_tree.txt"

echo ""
echo "Generating module dependency graph (library, requires Graphviz)..."
if command -v dot &> /dev/null; then
    cargo modules dependencies --lib | dot -Tpng > module_graph.png
    echo "Module graph saved to module_graph.png"
    
    # Also generate SVG version
    cargo modules dependencies --lib | dot -Tsvg > module_graph.svg
    echo "Module graph (SVG) saved to module_graph.svg"
else
    echo "Graphviz not found. Install with:"
    echo "  - Linux: sudo apt-get install graphviz"
    echo "  - Windows: choco install graphviz"
    echo "  - macOS: brew install graphviz"
    echo ""
    echo "Dependency graph (DOT format) saved to module_graph.dot"
    cargo modules dependencies --lib > module_graph.dot
fi

