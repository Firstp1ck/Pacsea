#!/usr/bin/env python3
"""
Script to update VersionAnnouncement entries in src/announcements.rs

What: Updates or adds a VersionAnnouncement entry in the VERSION_ANNOUNCEMENTS array.

Inputs:
- version: Version string (e.g., "0.6.0")
- title: Title of the announcement
- content: Markdown content of the announcement (can be multiline)

Output:
- Updates src/announcements.rs with the new or updated announcement

Details:
- If an announcement for the version already exists, it will be updated
- If not, a new announcement will be added to the array
- Preserves formatting and comments in the file
- Handles multiline content properly

Usage:
    ./update_version_announcement.py <version> <title> <content>
    ./update_version_announcement.py "0.6.0" "Welcome" "Content here"
    
    # With multiline content from file:
    ./update_version_announcement.py "0.6.0" "Welcome" "$(cat announcement.md)"
    
    # Interactive mode (version defaults to Cargo.toml version):
    ./update_version_announcement.py
"""

import re
import sys
import os
from pathlib import Path

# Path to the announcements file
ANNOUNCEMENTS_FILE = Path(__file__).parent.parent.parent / "src" / "announcements.rs"
# Path to Cargo.toml
CARGO_TOML_FILE = Path(__file__).parent.parent.parent / "Cargo.toml"


def get_version_from_cargo_toml() -> str | None:
    """
    Read version from Cargo.toml.
    
    Returns:
        Version string if found, None otherwise.
    """
    if not CARGO_TOML_FILE.exists():
        return None
    
    try:
        with open(CARGO_TOML_FILE, "r", encoding="utf-8") as f:
            content = f.read()
        
        # Look for version = "x.y.z" pattern
        pattern = r'version\s*=\s*"([^"]+)"'
        match = re.search(pattern, content)
        if match:
            return match.group(1)
    except Exception as err:
        print(
            f"Warning: failed to read version from {CARGO_TOML_FILE}: {err}",
            file=sys.stderr,
        )
    
    return None


def escape_rust_string(s: str) -> str:
    """Escape special characters for Rust string literals."""
    # Escape in the correct order: backslashes first, then quotes
    # But we need to be careful - if we have literal \n, we want to keep it as \n
    # So we replace backslashes first, then quotes
    result = []
    for char in s:
        if char == '\\':
            result.append('\\\\')
        elif char == '"':
            result.append('\\"')
        else:
            result.append(char)
    return ''.join(result)


def format_content_for_rust(content: str) -> str:
    """Format content as a Rust multiline string literal."""
    # Escape the content
    escaped = escape_rust_string(content)
    # Split into lines and format
    lines = escaped.split("\n")
    if len(lines) == 1:
        return f'"{lines[0]}"'
    
    # For multiline, use a string literal with \n
    # Join with \n and wrap in quotes
    return f'"{escaped}"'


def parse_announcements(content: str) -> tuple[list[dict], int, int]:
    """
    Parse existing announcements from the file.
    
    Returns:
        (announcements_list, start_line, end_line)
    """
    # Find the VERSION_ANNOUNCEMENTS array
    pattern = r'pub const VERSION_ANNOUNCEMENTS: &\[VersionAnnouncement\] = &\[(.*?)\];'
    match = re.search(pattern, content, re.DOTALL)
    
    if not match:
        raise ValueError("Could not find VERSION_ANNOUNCEMENTS array in file")
    
    array_content = match.group(1)
    start_pos = match.start()
    end_pos = match.end()
    
    # Find line numbers
    start_line = content[:start_pos].count("\n") + 1
    end_line = content[:end_pos].count("\n") + 1
    
    # Parse individual announcements
    announcements = []
    # Pattern to match VersionAnnouncement blocks
    # Use a simpler approach: match fields separately allowing for escaped sequences
    ann_pattern = (
        r'VersionAnnouncement\s*\{'
        r'[^}]*version:\s*"([^"]+)"'
        r'[^}]*title:\s*"([^"]+)"'
        r'[^}]*content:\s*"([^"]+)"'
        r'[^}]*\}'
    )
    
    for ann_match in re.finditer(ann_pattern, array_content, re.DOTALL):
        # Unescape the strings - handle \n, \\, and \"
        def unescape(s: str) -> str:
            result = []
            i = 0
            while i < len(s):
                if s[i] == '\\' and i + 1 < len(s):
                    if s[i + 1] == 'n':
                        result.append('\n')
                        i += 2
                    elif s[i + 1] == '\\':
                        result.append('\\')
                        i += 2
                    elif s[i + 1] == '"':
                        result.append('"')
                        i += 2
                    else:
                        # Unknown escape, keep as-is
                        result.append(s[i])
                        i += 1
                else:
                    result.append(s[i])
                    i += 1
            return ''.join(result)
        
        announcements.append({
            "version": unescape(ann_match.group(1)),
            "title": unescape(ann_match.group(2)),
            "content": unescape(ann_match.group(3)),
            "match": ann_match
        })
    
    return announcements, start_line, end_line


def update_announcement(content: str, version: str, title: str, announcement_content: str) -> str:
    """Update or add an announcement in the file content."""
    announcements, start_line, end_line = parse_announcements(content)
    
    # Check if version already exists
    existing_idx = None
    for idx, ann in enumerate(announcements):
        if ann["version"] == version:
            existing_idx = idx
            break
    
    # Update existing or add new
    if existing_idx is not None:
        announcements[existing_idx]["title"] = title
        announcements[existing_idx]["content"] = announcement_content
        print(f"Updating existing announcement for version {version}")
    else:
        announcements.append({
            "version": version,
            "title": title,
            "content": announcement_content
        })
        print(f"Adding new announcement for version {version}")
    
    # Rebuild the array content
    array_lines = ["    // Add version-specific announcements here"]
    
    for ann in announcements:
        # Escape strings properly
        escaped_version = escape_rust_string(ann["version"])
        escaped_title = escape_rust_string(ann["title"])
        # For content, handle newlines and other escapes:
        # 1. Convert actual newlines to \\n (double backslash + n) so when written to file it becomes \n
        # 2. Escape quotes
        formatted_content = ann["content"].replace("\n", "\\\\n").replace('"', '\\"')
        
        array_lines.append("    VersionAnnouncement {")
        array_lines.append(f'        version: "{escaped_version}",')
        array_lines.append(f'        title: "{escaped_title}",')
        array_lines.append(f'        content: "{formatted_content}",')
        array_lines.append("    },")
    
    # Replace the array content - preserve the opening bracket on its own line
    pattern = r'(pub const VERSION_ANNOUNCEMENTS: &\[VersionAnnouncement\] = &\[)(.*?)(\];)'
    replacement = r'\1\n' + "\n".join(array_lines) + "\n" + r'\3'
    
    new_content = re.sub(pattern, replacement, content, flags=re.DOTALL)
    
    return new_content


def main():
    """Main entry point."""
    dry_run = False
    args = sys.argv[1:]
    
    # Check for --dry-run flag
    if "--dry-run" in args:
        dry_run = True
        args.remove("--dry-run")
        print("DRY RUN MODE - No changes will be made to files")
    
    if len(args) == 0:
        # Interactive mode
        print("Interactive VersionAnnouncement Updater")
        print("=" * 50)
        
        # Get default version from Cargo.toml
        default_version = get_version_from_cargo_toml()
        if default_version:
            version_prompt = f"Version [{default_version}]: "
        else:
            version_prompt = "Version (e.g., 0.6.0): "
        
        version_input = input(version_prompt).strip()
        version = version_input if version_input else default_version
        
        if not version:
            print("Error: version is required")
            sys.exit(1)
        
        title = input("Title: ").strip()
        print("Content (multiline, end with Ctrl+D or empty line + Enter):")
        content_lines = []
        try:
            while True:
                line = input()
                content_lines.append(line)
        except EOFError:
            pass
        content = "\n".join(content_lines)
    elif len(args) == 3:
        version = args[0]
        title = args[1]
        # Check if content is a file path or actual content
        content_arg = args[2]
        if os.path.isfile(content_arg):
            # Read from file
            with open(content_arg, "r", encoding="utf-8") as f:
                content = f.read()
        else:
            # Use as-is (may contain \n escape sequences)
            content = content_arg.replace("\\n", "\n")
    elif len(args) == 4 and args[2] == "--file":
        # Explicit file mode: ./script.py version title --file content.md
        version = args[0]
        title = args[1]
        content_file = args[3]
        if not os.path.isfile(content_file):
            print(f"Error: File not found: {content_file}")
            sys.exit(1)
        with open(content_file, "r", encoding="utf-8") as f:
            content = f.read()
    else:
        print("Usage:")
        print("  ./update_version_announcement.py [--dry-run] <version> <title> <content>")
        print("  ./update_version_announcement.py [--dry-run] <version> <title> --file <file>")
        print("  ./update_version_announcement.py [--dry-run]  (for interactive mode)")
        print("")
        print("Examples:")
        print('  ./update_version_announcement.py "0.6.0" "Welcome" "Content here"')
        print('  ./update_version_announcement.py "0.6.0" "Welcome" --file announcement.md')
        print('  ./update_version_announcement.py --dry-run "0.6.0" "Welcome" "$(cat announcement.md)"')
        sys.exit(1)
    
    if not version or not title or not content:
        print("Error: version, title, and content are required")
        sys.exit(1)
    
    # Read the file
    if not ANNOUNCEMENTS_FILE.exists():
        print(f"Error: File not found: {ANNOUNCEMENTS_FILE}")
        sys.exit(1)
    
    with open(ANNOUNCEMENTS_FILE, "r", encoding="utf-8") as f:
        file_content = f.read()
    
    # Update the announcement
    try:
        new_content = update_announcement(file_content, version, title, content)
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)
    
    # Write back (or show diff in dry-run mode)
    if dry_run:
        print("\n" + "=" * 70)
        print("DRY RUN - Would update file with the following changes:")
        print("=" * 70)
        # Show a diff-like output
        import difflib
        old_lines = file_content.splitlines(keepends=True)
        new_lines = new_content.splitlines(keepends=True)
        diff = difflib.unified_diff(old_lines, new_lines, 
                                   fromfile=str(ANNOUNCEMENTS_FILE),
                                   tofile=str(ANNOUNCEMENTS_FILE),
                                   lineterm='')
        for line in diff:
            print(line, end='')
        print("\n" + "=" * 70)
    else:
        with open(ANNOUNCEMENTS_FILE, "w", encoding="utf-8") as f:
            f.write(new_content)
        print(f"Successfully updated {ANNOUNCEMENTS_FILE}")


if __name__ == "__main__":
    main()

