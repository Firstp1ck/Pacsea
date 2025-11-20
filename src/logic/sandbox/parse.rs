//! Parsing functions for .SRCINFO and PKGBUILD dependency extraction.

/// What: Parse dependencies from .SRCINFO content.
///
/// Inputs:
/// - `srcinfo`: Raw .SRCINFO file content.
///
/// Output:
/// - Returns a tuple of (depends, makedepends, checkdepends, optdepends) vectors.
pub(crate) fn parse_srcinfo_deps(
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
            if value.ends_with(".so") || value.contains(".so.") || value.contains(".so=") {
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
            let base_key = if let Some(stripped) = key.strip_suffix('+') {
                stripped
            } else {
                key
            };

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
                let deps = if let Some(closing_paren_pos) = find_matching_closing_paren(value) {
                    // Single-line array (may have content after closing paren): depends=('foo' 'bar') or depends+=('foo' 'bar') other_code
                    let array_content = &value[1..closing_paren_pos];
                    tracing::debug!("Parsing single-line {} array: {}", key, array_content);
                    parse_array_content(array_content)
                } else {
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

                        // Add this line to the array content
                        array_lines.push(next_line);
                    }

                    // Parse all collected lines as array content
                    let array_content = array_lines.join(" ");
                    tracing::debug!(
                        "Collected {} lines for multi-line {} array: {}",
                        array_lines.len(),
                        key,
                        array_content
                    );
                    parse_array_content(&array_content)
                };

                // Add dependencies to the appropriate vector (using base_key to handle both = and +=)
                match base_key {
                    "depends" => depends.extend(deps),
                    "makedepends" => makedepends.extend(deps),
                    "checkdepends" => checkdepends.extend(deps),
                    "optdepends" => optdepends.extend(deps),
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
