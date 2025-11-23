//! PKGBUILD parsing functions.

/// What: Parse backup array from PKGBUILD content.
///
/// Inputs:
/// - `pkgbuild`: Raw PKGBUILD file content.
///
/// Output:
/// - Returns a vector of backup file paths.
///
/// Details:
/// - Parses bash array syntax: `backup=('file1' 'file2' '/etc/config')`
/// - Handles single-line and multi-line array definitions.
#[must_use]
pub fn parse_backup_from_pkgbuild(pkgbuild: &str) -> Vec<String> {
    let mut backup_files = Vec::new();
    let mut in_backup_array = false;
    let mut current_line = String::new();

    for line in pkgbuild.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Look for backup= array declaration
        if line.starts_with("backup=") || line.starts_with("backup =") {
            in_backup_array = true;
            current_line = line.to_string();

            // Check if array is on single line: backup=('file1' 'file2')
            if let Some(start) = line.find('(')
                && let Some(end) = line.rfind(')')
            {
                let array_content = &line[start + 1..end];
                parse_backup_array_content(array_content, &mut backup_files);
                in_backup_array = false;
                current_line.clear();
            } else if line.contains('(') {
                // Multi-line array starting
                if let Some(start) = line.find('(') {
                    let array_content = &line[start + 1..];
                    parse_backup_array_content(array_content, &mut backup_files);
                }
            }
        } else if in_backup_array {
            // Continuation of multi-line array
            current_line.push(' ');
            current_line.push_str(line);

            // Check if array ends
            if line.contains(')') {
                if let Some(end) = line.rfind(')') {
                    let remaining = &line[..end];
                    parse_backup_array_content(remaining, &mut backup_files);
                }
                in_backup_array = false;
                current_line.clear();
            } else {
                // Still in array, parse this line
                parse_backup_array_content(line, &mut backup_files);
            }
        }
    }

    backup_files
}

/// What: Parse backup array content (handles quoted strings).
///
/// Inputs:
/// - `content`: String content containing quoted file paths.
/// - `backup_files`: Vector to append parsed file paths to.
///
/// Details:
/// - Extracts quoted strings (single or double quotes) from array content.
pub fn parse_backup_array_content(content: &str, backup_files: &mut Vec<String>) {
    let mut in_quotes = false;
    let mut quote_char = '\0';
    let mut current_file = String::new();

    for ch in content.chars() {
        match ch {
            '\'' | '"' => {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else if ch == quote_char {
                    // End of quoted string
                    if !current_file.is_empty() {
                        backup_files.push(current_file.clone());
                        current_file.clear();
                    }
                    in_quotes = false;
                    quote_char = '\0';
                } else {
                    // Different quote type, treat as part of string
                    current_file.push(ch);
                }
            }
            _ if in_quotes => {
                current_file.push(ch);
            }
            _ => {
                // Skip whitespace and other characters outside quotes
            }
        }
    }

    // Handle unclosed quote (edge case)
    if !current_file.is_empty() && in_quotes {
        backup_files.push(current_file);
    }
}

/// What: Parse backup array from .SRCINFO content.
///
/// Inputs:
/// - `srcinfo`: Raw .SRCINFO file content.
///
/// Output:
/// - Returns a vector of backup file paths.
///
/// Details:
/// - Parses key-value pairs: `backup = file1`
/// - Handles multiple backup entries.
#[must_use]
pub fn parse_backup_from_srcinfo(srcinfo: &str) -> Vec<String> {
    let mut backup_files = Vec::new();

    for line in srcinfo.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // .SRCINFO format: backup = file_path
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            if key == "backup" && !value.is_empty() {
                backup_files.push(value.to_string());
            }
        }
    }

    backup_files
}

/// What: Parse install paths from PKGBUILD content.
///
/// Inputs:
/// - `pkgbuild`: Raw PKGBUILD file content.
/// - `pkgname`: Package name (used for default install paths).
///
/// Output:
/// - Returns a vector of file paths that would be installed.
///
/// Details:
/// - Parses `package()` functions and `install` scripts to extract file paths.
/// - Handles common patterns like `install -Dm755`, `cp`, `mkdir -p`, etc.
/// - Extracts paths from `package()` functions that use `install` commands.
/// - This is a best-effort heuristic and may not capture all files.
#[must_use]
pub fn parse_install_paths_from_pkgbuild(pkgbuild: &str, pkgname: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut in_package_function = false;
    let mut package_function_depth = 0;

    for line in pkgbuild.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Detect package() function start
        if trimmed.starts_with("package()") || trimmed.starts_with("package_") {
            in_package_function = true;
            package_function_depth = 0;
            continue;
        }

        // Track function depth (handle nested functions)
        if in_package_function {
            if trimmed.contains('{') {
                package_function_depth += trimmed.matches('{').count();
            }
            if trimmed.contains('}') {
                let closing_count = trimmed.matches('}').count();
                if package_function_depth >= closing_count {
                    package_function_depth -= closing_count;
                } else {
                    package_function_depth = 0;
                }
                if package_function_depth == 0 {
                    in_package_function = false;
                    continue;
                }
            }

            // Parse install commands within package() function
            // Common patterns:
            // install -Dm755 "$srcdir/binary" "$pkgdir/usr/bin/binary"
            // install -Dm644 "$srcdir/config" "$pkgdir/etc/config"
            // cp -r "$srcdir/data" "$pkgdir/usr/share/app"

            if trimmed.contains("install") && trimmed.contains("$pkgdir") {
                // Extract destination path from install command
                // Pattern: install ... "$pkgdir/path/to/file"
                if let Some(pkgdir_pos) = trimmed.find("$pkgdir") {
                    let after_pkgdir = &trimmed[pkgdir_pos + 7..]; // Skip "$pkgdir"
                    // Find the path (may be quoted)
                    let path_start = after_pkgdir
                        .chars()
                        .position(|c| c != ' ' && c != '/' && c != '"' && c != '\'')
                        .unwrap_or(0);
                    let path_part = &after_pkgdir[path_start..];

                    // Extract path until space, quote, or end
                    let path_end = path_part
                        .chars()
                        .position(|c| c == ' ' || c == '"' || c == '\'' || c == ';')
                        .unwrap_or(path_part.len());

                    let mut path = path_part[..path_end].to_string();
                    // Remove leading slash if present (we'll add it)
                    if path.starts_with('/') {
                        path.remove(0);
                    }
                    if !path.is_empty() {
                        {
                            let path_str = &path;
                            files.push(format!("/{path_str}"));
                        }
                    }
                }
            } else if trimmed.contains("cp") && trimmed.contains("$pkgdir") {
                // Extract destination from cp command
                // Pattern: cp ... "$pkgdir/path/to/file"
                if let Some(pkgdir_pos) = trimmed.find("$pkgdir") {
                    let after_pkgdir = &trimmed[pkgdir_pos + 7..];
                    let path_start = after_pkgdir
                        .chars()
                        .position(|c| c != ' ' && c != '/' && c != '"' && c != '\'')
                        .unwrap_or(0);
                    let path_part = &after_pkgdir[path_start..];
                    let path_end = path_part
                        .chars()
                        .position(|c| c == ' ' || c == '"' || c == '\'' || c == ';')
                        .unwrap_or(path_part.len());

                    let mut path = path_part[..path_end].to_string();
                    if path.starts_with('/') {
                        path.remove(0);
                    }
                    if !path.is_empty() {
                        {
                            let path_str = &path;
                            files.push(format!("/{path_str}"));
                        }
                    }
                }
            }
        }
    }

    // Remove duplicates and sort
    files.sort();
    files.dedup();

    // If we didn't find any files, try to infer common paths based on package name
    if files.is_empty() {
        // Common default paths for AUR packages
        files.push(format!("/usr/bin/{pkgname}"));
        files.push(format!("/usr/share/{pkgname}"));
    }

    files
}
