#!/bin/bash
#
# Module Structure Visualization Generator
#
# This script generates visual dependency graphs for each module in the Pacsea project.
# It uses cargo-modules to analyze the Rust codebase and Graphviz to create visualizations.
#
# What it does:
#   1. Generates a text-based module tree showing the hierarchical structure of all modules
#   2. For each top-level module in src/ (app, events, i18n, index, install, logic, sources, state, theme, ui):
#      - Creates a focused dependency graph showing only that module's internal dependencies
#      - Generates three output formats:
#        * DOT format (module_graph.dot) - Graphviz source format
#        * PNG image (module_graph.png) - High-resolution raster image
#        * SVG image (module_graph.svg) - Scalable vector graphic optimized for performance
#
# Output structure:
#   dev/scripts/Modules/
#   ├── module_tree.txt          # Text tree of all modules
#   ├── app/
#   │   ├── module_graph.dot
#   │   ├── module_graph.png
#   │   └── module_graph.svg
#   ├── events/
#   │   └── ...
#   └── [other modules]/
#
# Requirements:
#   - cargo-modules (installed automatically if missing)
#   - Graphviz (dot command) - must be installed manually
#
# Performance optimizations:
#   - Uses orthogonal splines for simpler, faster-rendering paths
#   - Filters out external dependencies (--no-externs) to reduce complexity
#   - Optimized SVG settings for better browser/viewer performance
#   - Large graph size (40x40 inches) with high DPI for clarity
#

set -e

# Change to script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Set Modules output directory (create if it doesn't exist)
MODULES_DIR="$SCRIPT_DIR/Modules"
mkdir -p "$MODULES_DIR"

# Check if cargo-modules is installed
if ! command -v cargo-modules &> /dev/null; then
    echo "Installing cargo-modules..."
    cargo install cargo-modules
fi

# Change to project root (find Cargo.toml)
PROJECT_ROOT="$(cd "$SCRIPT_DIR" && while [ ! -f "Cargo.toml" ] && [ "$PWD" != "/" ]; do cd ..; done && pwd)"
if [ ! -f "$PROJECT_ROOT/Cargo.toml" ]; then
    echo "Error: Could not find Cargo.toml. Please run this script from the Pacsea project directory."
    exit 1
fi
cd "$PROJECT_ROOT"

echo "Generating module tree (library)..."
cargo modules structure --lib > "$MODULES_DIR/module_tree.txt"
echo "Module tree saved to $MODULES_DIR/module_tree.txt"

echo ""
echo "Generating module dependency graphs for each subfolder (requires Graphviz)..."
if ! command -v dot &> /dev/null; then
    echo "Graphviz not found. Install with:"
    echo "  - Linux: sudo pacman -Sgraphviz"
    echo "  - Windows: choco install graphviz"
    echo "  - macOS: brew install graphviz"
    exit 1
fi

# List of subfolders in src/ to generate graphs for
SUBFOLDERS=("app" "events" "i18n" "index" "install" "logic" "sources" "state" "theme" "ui")

# Function to generate graph for a module
generate_module_graph() {
    local module_name=$1
    local focus_path="pacsea::$module_name"
    
    # Create directory for this module in Modules/
    local module_dir="$MODULES_DIR/$module_name"
    mkdir -p "$module_dir"
    
    echo ""
    echo "Generating graph for module: $module_name"
    echo "  Output directory: $module_dir"
    
    # Generate DOT format
    local dot_file="$module_dir/module_graph.dot"
    cargo modules dependencies --lib --focus-on "$focus_path" --no-externs > "$dot_file" 2>/dev/null || {
        echo "  Warning: Failed to generate DOT for $module_name, skipping..."
        rmdir "$module_dir" 2>/dev/null || true
        return 1
    }
    
    # Check if DOT file has content (more than just header)
    if [ ! -s "$dot_file" ] || [ "$(wc -l < "$dot_file")" -lt 3 ]; then
        echo "  No dependencies found for $module_name, skipping graph generation..."
        rm -f "$dot_file"
        rmdir "$module_dir" 2>/dev/null || true
        return 1
    fi
    
    # Generate PNG with larger size and improved readability
    # Increased size while maintaining performance with simple rendering
    if dot -Tpng \
        -Gdpi=200 \
        -Gsize=40,40 \
        -Gratio=compress \
        -Goverlap=prism \
        -Gsplines=ortho \
        -Gnodesep=2.0 \
        -Granksep=2.5 \
        -Gpad=0.8 \
        -Nfontsize=13 \
        -Nfontname="Arial" \
        -Nwidth=0 \
        -Nheight=0.35 \
        -Nmargin=0.2,0.12 \
        -Nstyle="rounded,filled" \
        -Nfillcolor="#f8f8f8" \
        -Ncolor="#333333" \
        -Epenwidth=1.5 \
        -Ecolor="#666666" \
        -Earrowsize=0.8 \
        -Elabeldistance=2.5 \
        "$dot_file" > "$module_dir/module_graph.png" 2>/dev/null; then
        echo "  PNG saved: $module_dir/module_graph.png"
    fi
    
    # Generate optimized SVG version - larger size with performance optimizations
    # Performance tips: ortho splines (simpler paths), simple shapes, no gradients
    # Larger size achieved through increased DPI and graph dimensions
    if dot -Tsvg \
        -Gdpi=120 \
        -Gsize=40,40 \
        -Gratio=compress \
        -Goverlap=prism \
        -Gsplines=ortho \
        -Gnodesep=2.0 \
        -Granksep=2.5 \
        -Gpad=0.8 \
        -Nfontsize=12 \
        -Nfontname="Arial" \
        -Nwidth=0 \
        -Nheight=0.35 \
        -Nmargin=0.2,0.12 \
        -Nshape=box \
        -Nstyle="rounded,filled" \
        -Nfillcolor="#f8f8f8" \
        -Ncolor="#333333" \
        -Epenwidth=1.2 \
        -Ecolor="#666666" \
        -Earrowsize=0.7 \
        -Elabeldistance=2.5 \
        "$dot_file" > "$module_dir/module_graph.svg" 2>/dev/null; then
        echo "  SVG saved: $module_dir/module_graph.svg"
    fi
}

# Generate graphs for each subfolder
for folder in "${SUBFOLDERS[@]}"; do
    generate_module_graph "$folder"
done

echo ""
echo "All module graphs generated in subdirectories of: $MODULES_DIR"
echo "Each module has its own folder containing: module_graph.dot, module_graph.png, module_graph.svg"

