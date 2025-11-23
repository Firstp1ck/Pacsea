//! Parsing functions for .SRCINFO and PKGBUILD dependency extraction.

/// What: Parse dependencies from .SRCINFO content.
///
/// Inputs:
/// - `srcinfo`: Raw .SRCINFO file content.
///
/// Output:
/// - Returns a tuple of (depends, makedepends, checkdepends, optdepends) vectors.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
pub(super) fn parse_srcinfo_deps(
    srcinfo: &str,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut depends = Vec::new();
    let mut makedepends = Vec::new();
    let mut checkdepends = Vec::new();
    let mut optdepends = Vec::new();

    for line in srcinfo.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // .SRCINFO format: key = value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            // Filter out virtual packages (.so files)
            let value_lower = value.to_lowercase();
            if value_lower.ends_with(".so")
                || value_lower.contains(".so.")
                || value_lower.contains(".so=")
            {
                continue;
            }

            match key {
                "depends" => depends.push(value.to_string()),
                "makedepends" => makedepends.push(value.to_string()),
                "checkdepends" => checkdepends.push(value.to_string()),
                "optdepends" => optdepends.push(value.to_string()),
                _ => {}
            }
        }
    }

    (depends, makedepends, checkdepends, optdepends)
}

/// What: Parse dependencies from PKGBUILD content.
///
/// Inputs:
/// - `pkgbuild`: Raw PKGBUILD file content.
///
/// Output:
/// - Returns a tuple of (depends, makedepends, checkdepends, optdepends) vectors.
///
/// Details:
/// - Parses bash array syntax: `depends=('foo' 'bar>=1.2')` (single-line)
/// - Also handles `depends+=` patterns used in functions like `package()`
/// - Handles both quoted and unquoted dependencies
/// - Also handles multi-line arrays:
///   ```text
///   depends=(
///       'foo'
///       'bar>=1.2'
///   )
///   ```
/// - Filters out .so files (virtual packages) and invalid package names
/// - Only parses specific dependency fields (depends, makedepends, checkdepends, optdepends)
pub fn parse_pkgbuild_deps(pkgbuild: &str) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    tracing::debug!(
        "parse_pkgbuild_deps: Starting parse, PKGBUILD length={}, first 500 chars: {:?}",
        pkgbuild.len(),
        pkgbuild.chars().take(500).collect::<String>()
    );
    let mut depends = Vec::new();
    let mut makedepends = Vec::new();
    let mut checkdepends = Vec::new();
    let mut optdepends = Vec::new();

    let lines: Vec<&str> = pkgbuild.lines().collect();
    tracing::debug!(
        "parse_pkgbuild_deps: Total lines in PKGBUILD: {}",
        lines.len()
    );
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        i += 1;

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse array declarations: depends=('foo' 'bar') or depends=( or depends+=('foo' 'bar')
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            // Handle both depends= and depends+= patterns
            let base_key = key.strip_suffix('+').map_or(key, |stripped| stripped);

            // Only parse specific dependency fields, ignore other PKGBUILD fields
            if !matches!(
                base_key,
                "depends" | "makedepends" | "checkdepends" | "optdepends"
            ) {
                continue;
            }

            tracing::debug!(
                "parse_pkgbuild_deps: Found key-value pair: key='{}', base_key='{}', value='{}'",
                key,
                base_key,
                value.chars().take(100).collect::<String>()
            );

            // Check if this is an array declaration
            if value.starts_with('(') {
                tracing::debug!(
                    "parse_pkgbuild_deps: Detected array declaration for key='{}'",
                    key
                );
                let deps = find_matching_closing_paren(value).map_or_else(
                    || {
                        // Multi-line array: depends=(
                        //     'foo'
                        //     'bar'
                        // )
                        tracing::debug!("Parsing multi-line {} array", key);
                        let mut array_lines = Vec::new();
                        // Collect lines until we find the closing parenthesis
                        while i < lines.len() {
                            let next_line = lines[i].trim();
                            i += 1;

                            // Skip empty lines and comments
                            if next_line.is_empty() || next_line.starts_with('#') {
                                continue;
                            }

                            // Check if this line closes the array
                            if next_line == ")" {
                                break;
                            }

                            // Check if this line contains a closing parenthesis (may be on same line as content)
                            if let Some(paren_pos) = next_line.find(')') {
                                // Extract content before the closing paren
                                let content_before_paren = &next_line[..paren_pos].trim();
                                if !content_before_paren.is_empty() {
                                    array_lines.push((*content_before_paren).to_string());
                                }
                                break;
                            }

                            // Add this line to the array content
                            array_lines.push(next_line.to_string());
                        }

                        // Parse all collected lines as array content
                        // Ensure proper spacing between items (each line should be a separate item)
                        let array_content = array_lines
                            .iter()
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ");
                        tracing::debug!(
                            "Collected {} lines for multi-line {} array: {}",
                            array_lines.len(),
                            key,
                            array_content
                        );
                        let parsed = parse_array_content(&array_content);
                        tracing::debug!("Parsed array content: {:?}", parsed);
                        parsed
                    },
                    |closing_paren_pos| {
                        // Single-line array (may have content after closing paren): depends=('foo' 'bar') or depends+=('foo' 'bar') other_code
                        let array_content = &value[1..closing_paren_pos];
                        tracing::debug!("Parsing single-line {} array: {}", key, array_content);
                        let parsed = parse_array_content(array_content);
                        tracing::debug!("Parsed array content: {:?}", parsed);
                        parsed
                    },
                );

                // Filter out invalid dependencies (.so files, invalid names, etc.)
                let filtered_deps: Vec<String> = deps
                    .into_iter()
                    .filter_map(|dep| {
                        let dep_trimmed = dep.trim();
                        if dep_trimmed.is_empty() {
                            return None;
                        }

                        // Filter out .so files (virtual packages)
                        let dep_lower = dep_trimmed.to_lowercase();
                        if std::path::Path::new(&dep_lower)
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("so"))
                            || dep_lower.contains(".so.")
                            || dep_lower.contains(".so=")
                        {
                            return None;
                        }

                        // Filter out names ending with ) - this is a parsing error
                        // But first check if it's actually a valid name with version constraint ending in )
                        // like "package>=1.0)" which would be a parsing error
                        if dep_trimmed.ends_with(')') {
                            // Check if it might be a valid version constraint that accidentally ends with )
                            // If it contains version operators before the ), it's likely a parsing error
                            if dep_trimmed.contains(">=")
                                || dep_trimmed.contains("<=")
                                || dep_trimmed.contains("==")
                            {
                                // This looks like "package>=1.0)" which is invalid
                                return None;
                            }
                            // Otherwise, it might be "package)" which is also invalid
                            return None;
                        }

                        // Filter out names that don't look like package names
                        // Package names should start with alphanumeric or underscore
                        let first_char = dep_trimmed.chars().next().unwrap_or(' ');
                        if !first_char.is_alphanumeric() && first_char != '_' {
                            return None;
                        }

                        // Filter out names that are too short
                        if dep_trimmed.len() < 2 {
                            return None;
                        }

                        // Filter out names containing invalid characters (but allow version operators)
                        // Allow: alphanumeric, dash, underscore, and version operators (>=, <=, ==, >, <)
                        let has_valid_chars = dep_trimmed
                            .chars()
                            .any(|c| c.is_alphanumeric() || c == '-' || c == '_');
                        if !has_valid_chars {
                            return None;
                        }

                        Some(dep_trimmed.to_string())
                    })
                    .collect();

                // Add dependencies to the appropriate vector (using base_key to handle both = and +=)
                match base_key {
                    "depends" => depends.extend(filtered_deps),
                    "makedepends" => makedepends.extend(filtered_deps),
                    "checkdepends" => checkdepends.extend(filtered_deps),
                    "optdepends" => optdepends.extend(filtered_deps),
                    _ => {}
                }
            }
        }
    }

    (depends, makedepends, checkdepends, optdepends)
}

/// What: Find the position of the matching closing parenthesis in a string.
///
/// Inputs:
/// - `s`: String starting with an opening parenthesis.
///
/// Output:
/// - `Some(position)` if a matching closing parenthesis is found, `None` otherwise.
///
/// Details:
/// - Handles nested parentheses and quoted strings.
fn find_matching_closing_paren(s: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_quotes = false;
    let mut quote_char = '\0';

    for (pos, ch) in s.char_indices() {
        match ch {
            '\'' | '"' => {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else if ch == quote_char {
                    in_quotes = false;
                    quote_char = '\0';
                }
            }
            '(' if !in_quotes => {
                depth += 1;
            }
            ')' if !in_quotes => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            _ => {}
        }
    }
    None
}

/// What: Parse quoted and unquoted strings from bash array content.
///
/// Inputs:
/// - `content`: Array content string (e.g., "'foo' 'bar>=1.2'" or "libcairo.so libdbus-1.so").
///
/// Output:
/// - Vector of dependency strings.
///
/// Details:
/// - Handles both quoted ('foo') and unquoted (foo) dependencies.
/// - Splits on whitespace for unquoted values.
fn parse_array_content(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';
    let mut current = String::new();

    for ch in content.chars() {
        match ch {
            '\'' | '"' => {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else if ch == quote_char {
                    if !current.is_empty() {
                        deps.push(current.clone());
                        current.clear();
                    }
                    in_quotes = false;
                    quote_char = '\0';
                } else {
                    current.push(ch);
                }
            }
            _ if in_quotes => {
                current.push(ch);
            }
            ch if ch.is_whitespace() => {
                // Whitespace outside quotes - end current unquoted value
                if !current.is_empty() {
                    deps.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                // Non-whitespace character outside quotes - add to current value
                current.push(ch);
            }
        }
    }

    // Handle unclosed quote or trailing unquoted value
    if !current.is_empty() {
        deps.push(current);
    }

    deps
}

/// What: Parse conflicts from PKGBUILD content.
///
/// Inputs:
/// - `pkgbuild`: Raw PKGBUILD file content.
///
/// Output:
/// - Returns a vector of conflicting package names.
///
/// Details:
/// - Parses bash array syntax: `conflicts=('foo' 'bar')` (single-line)
/// - Also handles `conflicts+=` patterns used in functions like `package()`
/// - Handles both quoted and unquoted conflicts
/// - Also handles multi-line arrays:
///   ```text
///   conflicts=(
///       'foo'
///       'bar'
///   )
///   ```
/// - Filters out .so files (virtual packages) and invalid package names
/// - Extracts package names from version constraints (e.g., "jujutsu-git>=1.0" -> "jujutsu-git")
pub fn parse_pkgbuild_conflicts(pkgbuild: &str) -> Vec<String> {
    tracing::debug!(
        "parse_pkgbuild_conflicts: Starting parse, PKGBUILD length={}",
        pkgbuild.len()
    );
    let mut conflicts = Vec::new();

    let lines: Vec<&str> = pkgbuild.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        i += 1;

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse array declarations: conflicts=('foo' 'bar') or conflicts=( or conflicts+=('foo' 'bar')
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            // Handle both conflicts= and conflicts+= patterns
            let base_key = key.strip_suffix('+').map_or(key, |stripped| stripped);

            // Only parse conflicts field
            if base_key != "conflicts" {
                continue;
            }

            tracing::debug!(
                "parse_pkgbuild_conflicts: Found key-value pair: key='{}', base_key='{}', value='{}'",
                key,
                base_key,
                value.chars().take(100).collect::<String>()
            );

            // Check if this is an array declaration
            if value.starts_with('(') {
                tracing::debug!(
                    "parse_pkgbuild_conflicts: Detected array declaration for key='{}'",
                    key
                );
                let conflict_deps = find_matching_closing_paren(value).map_or_else(
                    || {
                        // Multi-line array: conflicts=(
                        //     'foo'
                        //     'bar'
                        // )
                        tracing::debug!("Parsing multi-line {} array", key);
                        let mut array_lines = Vec::new();
                        // Collect lines until we find the closing parenthesis
                        while i < lines.len() {
                            let next_line = lines[i].trim();
                            i += 1;

                            // Skip empty lines and comments
                            if next_line.is_empty() || next_line.starts_with('#') {
                                continue;
                            }

                            // Check if this line closes the array
                            if next_line == ")" {
                                break;
                            }

                            // Check if this line contains a closing parenthesis (may be on same line as content)
                            if let Some(paren_pos) = next_line.find(')') {
                                // Extract content before the closing paren
                                let content_before_paren = &next_line[..paren_pos].trim();
                                if !content_before_paren.is_empty() {
                                    array_lines.push((*content_before_paren).to_string());
                                }
                                break;
                            }

                            // Add this line to the array content
                            array_lines.push(next_line.to_string());
                        }

                        // Parse all collected lines as array content
                        let array_content = array_lines
                            .iter()
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ");
                        tracing::debug!(
                            "Collected {} lines for multi-line {} array: {}",
                            array_lines.len(),
                            key,
                            array_content
                        );
                        let parsed = parse_array_content(&array_content);
                        tracing::debug!("Parsed array content: {:?}", parsed);
                        parsed
                    },
                    |closing_paren_pos| {
                        // Single-line array (may have content after closing paren): conflicts=('foo' 'bar') or conflicts+=('foo' 'bar') other_code
                        let array_content = &value[1..closing_paren_pos];
                        tracing::debug!("Parsing single-line {} array: {}", key, array_content);
                        let parsed = parse_array_content(array_content);
                        tracing::debug!("Parsed array content: {:?}", parsed);
                        parsed
                    },
                );

                // Filter out invalid conflicts (.so files, invalid names, etc.)
                let filtered_conflicts: Vec<String> = conflict_deps
                    .into_iter()
                    .filter_map(|conflict| {
                        let conflict_trimmed = conflict.trim();
                        if conflict_trimmed.is_empty() {
                            return None;
                        }

                        // Filter out .so files (virtual packages)
                        let conflict_lower = conflict_trimmed.to_lowercase();
                        if std::path::Path::new(&conflict_lower)
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("so"))
                            || conflict_lower.contains(".so.")
                            || conflict_lower.contains(".so=")
                        {
                            return None;
                        }

                        // Filter out names ending with ) - this is a parsing error
                        if conflict_trimmed.ends_with(')') {
                            return None;
                        }

                        // Filter out names that don't look like package names
                        let first_char = conflict_trimmed.chars().next().unwrap_or(' ');
                        if !first_char.is_alphanumeric() && first_char != '_' {
                            return None;
                        }

                        // Filter out names that are too short
                        if conflict_trimmed.len() < 2 {
                            return None;
                        }

                        // Filter out names containing invalid characters (but allow version operators)
                        let has_valid_chars = conflict_trimmed
                            .chars()
                            .any(|c| c.is_alphanumeric() || c == '-' || c == '_');
                        if !has_valid_chars {
                            return None;
                        }

                        // Extract package name (remove version constraints if present)
                        // Use a simple approach: split on version operators
                        let pkg_name = conflict_trimmed.find(['>', '<', '=']).map_or_else(
                            || conflict_trimmed.to_string(),
                            |pos| conflict_trimmed[..pos].trim().to_string(),
                        );
                        if pkg_name.is_empty() {
                            None
                        } else {
                            Some(pkg_name)
                        }
                    })
                    .collect();

                // Add conflicts to the vector (using base_key to handle both = and +=)
                conflicts.extend(filtered_conflicts);
            }
        }
    }

    conflicts
}
