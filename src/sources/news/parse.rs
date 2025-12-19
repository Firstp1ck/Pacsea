//! HTML parsing and rendering for news content.

use crate::sources::news::utils::{extract_origin, is_arch_package_url, resolve_href};
use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node, Selector};

/// What: Parse Arch Linux news HTML and extract article text using `scraper`.
///
/// Inputs:
/// - `html`: Raw HTML content of the news page.
///
/// Output:
/// - Extracted article text with formatting preserved (paragraphs, bullets, code markers).
pub fn parse_arch_news_html(html: &str, base_url: Option<&str>) -> String {
    let document = Html::parse_document(html);
    let base_origin = base_url.and_then(extract_origin);
    let is_pkg_page = base_url.is_some_and(is_arch_package_url);
    let selectors = [
        Selector::parse("div.advisory").ok(),
        Selector::parse("div.article-content").ok(),
        Selector::parse("article").ok(),
    ];

    let mut buf = String::new();
    let mut found = false;
    for sel in selectors.iter().flatten() {
        if let Some(element) = document.select(sel).next()
            && let Some(node) = document.tree.get(element.id())
        {
            let preserve_ws = element
                .value()
                .attr("class")
                .is_some_and(|c| c.contains("advisory"));
            render_node(&mut buf, node, false, preserve_ws, base_origin.as_deref());
            found = true;
            break;
        }
    }
    if !found && let Some(root) = document.tree.get(document.root_element().id()) {
        render_node(&mut buf, root, false, false, base_origin.as_deref());
    }

    let main = prune_news_boilerplate(&buf);
    if !is_pkg_page {
        return main;
    }

    let meta_block = extract_package_metadata(&document, base_origin.as_deref());
    if meta_block.is_empty() {
        return main;
    }

    let mut combined = String::new();
    combined.push_str("Package Info:\n");
    for line in meta_block {
        combined.push_str(&line);
        combined.push('\n');
    }
    combined.push('\n');
    combined.push_str(&main);
    combined
}

/// What: Render a node (and children) into text while preserving basic formatting.
///
/// Inputs:
/// - `buf`: Output buffer to append text into
/// - `node`: Node to render
/// - `in_pre`: Whether we are inside a <pre> block (preserve whitespace)
/// - `preserve_ws`: Whether to avoid collapsing whitespace (advisory pages).
fn render_node(
    buf: &mut String,
    node: NodeRef<Node>,
    in_pre: bool,
    preserve_ws: bool,
    base_origin: Option<&str>,
) {
    match node.value() {
        Node::Text(t) => push_text(buf, t.as_ref(), in_pre, preserve_ws),
        Node::Element(el) => {
            let name = el.name();
            let is_block = matches!(
                name,
                "p" | "div"
                    | "section"
                    | "article"
                    | "header"
                    | "footer"
                    | "main"
                    | "table"
                    | "tr"
                    | "td"
            );
            let is_list = matches!(name, "ul" | "ol");
            let is_li = name == "li";
            let is_br = name == "br";
            let is_pre_tag = name == "pre";
            let is_code = name == "code";
            let is_anchor = name == "a";

            if is_block && !buf.ends_with('\n') {
                buf.push('\n');
            }
            if is_li {
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
                buf.push_str("• ");
            }
            if is_br {
                buf.push('\n');
            }

            if is_anchor {
                let mut tmp = String::new();
                for child in node.children() {
                    render_node(&mut tmp, child, in_pre, preserve_ws, base_origin);
                }
                let label = tmp.trim();
                let href = el
                    .attr("href")
                    .map(str::trim)
                    .filter(|h| !h.is_empty())
                    .unwrap_or_default();
                if !href.is_empty() {
                    if !buf.ends_with('\n') && !buf.ends_with(' ') {
                        buf.push(' ');
                    }
                    if label.is_empty() {
                        buf.push_str(&resolve_href(href, base_origin));
                    } else {
                        buf.push_str(label);
                        buf.push(' ');
                        buf.push('(');
                        buf.push_str(&resolve_href(href, base_origin));
                        buf.push(')');
                    }
                } else if !label.is_empty() {
                    buf.push_str(label);
                }
                return;
            }

            if is_code {
                let mut tmp = String::new();
                for child in node.children() {
                    render_node(&mut tmp, child, in_pre, preserve_ws, base_origin);
                }
                if !tmp.is_empty() {
                    if !buf.ends_with('`') {
                        buf.push('`');
                    }
                    buf.push_str(tmp.trim());
                    buf.push('`');
                }
                return;
            }

            if is_pre_tag {
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
                let mut tmp = String::new();
                for child in node.children() {
                    render_node(&mut tmp, child, true, preserve_ws, base_origin);
                }
                buf.push_str(tmp.trim_end());
                buf.push('\n');
                return;
            }

            let next_pre = in_pre;
            for child in node.children() {
                render_node(buf, child, next_pre, preserve_ws, base_origin);
            }

            if is_block || is_list || is_li {
                if !buf.ends_with('\n') {
                    buf.push('\n');
                }
                if !buf.ends_with("\n\n") {
                    buf.push('\n');
                }
            }
        }
        _ => {}
    }
}

/// What: Append text content to buffer, preserving whitespace when in <pre>, otherwise collapsing runs.
///
/// Inputs:
/// - `buf`: Output buffer to append into.
/// - `text`: Text content from the node.
/// - `in_pre`: Whether whitespace should be preserved (inside `<pre>`).
/// - `preserve_ws`: Whether to avoid collapsing whitespace for advisory pages.
///
/// Output:
/// - Mutates `buf` with appended text respecting whitespace rules.
fn push_text(buf: &mut String, text: &str, in_pre: bool, preserve_ws: bool) {
    if in_pre {
        buf.push_str(text);
        return;
    }
    if preserve_ws {
        buf.push_str(text);
        return;
    }

    // Collapse consecutive whitespace to a single space, but keep newlines produced by block tags.
    let mut last_was_space = buf.ends_with(' ');
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                buf.push(' ');
                last_was_space = true;
            }
        } else {
            buf.push(ch);
            last_was_space = false;
        }
    }
}

/// What: Remove Arch news boilerplate (nav/header) from extracted text.
///
/// Inputs:
/// - `text`: Plain text extracted from the news HTML.
///
/// Output:
/// - Text with leading navigation/header lines removed, starting after the date line when found.
pub fn prune_news_boilerplate(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    // Find a date line like YYYY-MM-DD ...
    let date_idx = lines.iter().position(|l| {
        let t = l.trim();
        t.len() >= 10
            && t.as_bytes().get(4) == Some(&b'-')
            && t.as_bytes().get(7) == Some(&b'-')
            && t[..4].chars().all(|c| c.is_ascii_digit())
            && t[5..7].chars().all(|c| c.is_ascii_digit())
            && t[8..10].chars().all(|c| c.is_ascii_digit())
    });

    if let Some(idx) = date_idx {
        // Take everything after the date line
        let mut out: Vec<&str> = lines.iter().skip(idx + 1).map(|s| s.trim_end()).collect();
        // Drop leading empty lines
        while matches!(out.first(), Some(l) if l.trim().is_empty()) {
            out.remove(0);
        }
        // Drop footer/copyright block if present
        if let Some(c_idx) = out.iter().position(|l| l.contains("Copyright \u{00a9}")) {
            out.truncate(c_idx);
        }
        // Also drop known footer lines
        out.retain(|l| {
            let t = l.trim();
            !(t.starts_with("The Arch Linux name and logo")
                || t.starts_with("trademarks.")
                || t.starts_with("The registered trademark")
                || t.starts_with("Linux\u{00ae} is used")
                || t.starts_with("the exclusive licensee"))
        });
        return collapse_blank_lines(&out);
    }

    // Advisory pages don't match the date format; drop leading navigation until the first meaningful header
    let mut start = lines
        .iter()
        .position(|l| {
            let t = l.trim();
            t.starts_with("Arch Linux Security Advisory")
                || t.starts_with("Severity:")
                || t.starts_with("CVE-")
        })
        .unwrap_or(0);
    while start < lines.len() && {
        let t = lines[start].trim();
        t.is_empty() || t.starts_with('•') || t == "Arch Linux"
    } {
        start += 1;
    }
    let mut out: Vec<&str> = lines
        .iter()
        .skip(start)
        .map(|s| s.trim_end_matches('\r'))
        .collect();
    while matches!(out.first(), Some(l) if l.trim().is_empty() || l.trim().starts_with('•')) {
        out.remove(0);
    }
    collapse_blank_lines(&out)
}

/// What: Collapse multiple consecutive blank lines into a single blank line and trim trailing blanks.
pub fn collapse_blank_lines(lines: &[&str]) -> String {
    let mut out = Vec::with_capacity(lines.len());
    let mut last_was_blank = false;
    for l in lines {
        let blank = l.trim().is_empty();
        if blank && last_was_blank {
            continue;
        }
        out.push(l.trim_end());
        last_was_blank = blank;
    }
    while matches!(out.last(), Some(l) if l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// What: Extract selected metadata fields from an Arch package HTML page.
fn extract_package_metadata(document: &Html, base_origin: Option<&str>) -> Vec<String> {
    let wanted = [
        "Upstream URL",
        "License(s)",
        "Maintainers",
        "Package Size",
        "Installed Size",
        "Last Packager",
        "Build Date",
    ];
    let wanted_set: std::collections::HashSet<&str> = wanted.into_iter().collect();
    let row_sel = Selector::parse("tr").ok();
    let th_sel = Selector::parse("th").ok();
    let td_selector = Selector::parse("td").ok();
    let dt_sel = Selector::parse("dt").ok();
    let dd_selector = Selector::parse("dd").ok();
    let mut fields: Vec<(String, String)> = Vec::new();
    if let (Some(row_sel), Some(th_sel), Some(td_sel)) = (row_sel, th_sel, td_selector) {
        for tr in document.select(&row_sel) {
            let th_text = normalize_label(
                &tr.select(&th_sel)
                    .next()
                    .map(|th| th.text().collect::<String>())
                    .unwrap_or_default(),
            );
            if !wanted_set.contains(th_text.as_str()) {
                continue;
            }
            if let Some(td) = tr.select(&td_sel).next() {
                let value = extract_inline(&td, base_origin);
                if !value.is_empty() {
                    fields.push((th_text, value));
                }
            }
        }
    }
    if let (Some(dt_sel), Some(_dd_sel)) = (dt_sel, dd_selector) {
        for dt in document.select(&dt_sel) {
            let label = normalize_label(&dt.text().collect::<String>());
            if !wanted_set.contains(label.as_str()) {
                continue;
            }
            // Prefer the immediate following sibling <dd>
            if let Some(dd) = dt
                .next_sibling()
                .and_then(ElementRef::wrap)
                .filter(|sib| sib.value().name() == "dd")
                .or_else(|| dt.next_siblings().find_map(ElementRef::wrap))
            {
                let value = extract_inline(&dd, base_origin);
                if !value.is_empty() {
                    fields.push((label, value));
                }
            }
        }
    }
    fields
        .into_iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect()
}

/// What: Extract inline text (with resolved links) from a node subtree.
fn extract_inline(node: &NodeRef<Node>, base_origin: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for child in node.children() {
        match child.value() {
            Node::Text(t) => {
                let text = t.trim();
                if !text.is_empty() {
                    parts.push(text.to_string());
                }
            }
            Node::Element(el) => {
                if el.name() == "a" {
                    let label = ElementRef::wrap(child)
                        .map(|e| e.text().collect::<String>())
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    let href = el
                        .attr("href")
                        .map(str::trim)
                        .filter(|h| !h.is_empty())
                        .map(|h| resolve_href(h, base_origin))
                        .unwrap_or_default();
                    if !label.is_empty() && !href.is_empty() {
                        parts.push(format!("{label} ({href})"));
                    } else if !label.is_empty() {
                        parts.push(label);
                    } else if !href.is_empty() {
                        parts.push(href);
                    }
                } else {
                    let inline = extract_inline(&child, base_origin);
                    if !inline.is_empty() {
                        parts.push(inline);
                    }
                }
            }
            _ => {}
        }
    }
    parts
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// What: Normalize table/header labels for matching (trim and drop trailing colon).
fn normalize_label(raw: &str) -> String {
    raw.trim().trim_end_matches(':').trim().to_string()
}
