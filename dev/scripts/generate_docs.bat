@echo off
REM Generate Rust documentation with dependency graphs

echo Generating Rust documentation...
cargo doc --no-deps --document-private-items

echo.
echo Documentation generated in target/doc/
echo Open with: cargo doc --open

REM Optional: Generate dependency tree
echo.
echo Dependency tree:
cargo tree --depth 2

