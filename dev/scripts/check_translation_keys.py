#!/usr/bin/env python3
"""Check for missing and extra translation keys between locale files.

This script compares a target locale file against the English (en-US) locale file
and reports:
- Missing keys (keys in English but not in target) - treated as errors
- Extra keys (keys in target but not in English) - treated as warnings

Usage:
    python3 check_translation_keys.py <target_locale>
    
Examples:
    python3 check_translation_keys.py hu-HU
    python3 check_translation_keys.py de-DE
"""

import sys
import yaml
from pathlib import Path


def flatten_dict(d, parent_key='', sep='.'):
    """Flatten a nested dictionary into dot-notation keys.
    
    Args:
        d: Dictionary to flatten
        parent_key: Parent key prefix (for recursion)
        sep: Separator for keys (default: '.')
        
    Returns:
        Dictionary with flattened keys
    """
    items = []
    for k, v in d.items():
        new_key = f'{parent_key}{sep}{k}' if parent_key else k
        if isinstance(v, dict):
            items.extend(flatten_dict(v, new_key, sep=sep).items())
        elif isinstance(v, list):
            items.append((new_key, v))
        else:
            items.append((new_key, v))
    return dict(items)


def find_locales_dir():
    """Find the locales directory.
    
    Returns:
        Path to locales directory or None if not found
    """
    # Try development location first
    dev_path = Path(__file__).parent.parent.parent / 'config' / 'locales'
    if dev_path.exists() and dev_path.is_dir():
        return dev_path
    
    # Try installed location
    installed_path = Path('/usr/share/pacsea/locales')
    if installed_path.exists() and installed_path.is_dir():
        return installed_path
    
    return None


def main():
    """Main function to check for missing translation keys."""
    if len(sys.argv) < 2:
        print("Usage: python3 check_translation_keys.py <target_locale>")
        print("Example: python3 check_translation_keys.py hu-HU")
        sys.exit(1)
    
    target_locale = sys.argv[1]
    
    # Find locales directory
    locales_dir = find_locales_dir()
    if locales_dir is None:
        print("Error: Could not find locales directory")
        sys.exit(1)
    
    en_file = locales_dir / 'en-US.yml'
    target_file = locales_dir / f'{target_locale}.yml'
    
    # Check if files exist
    if not en_file.exists():
        print(f"Error: English locale file not found: {en_file}")
        sys.exit(1)
    
    if not target_file.exists():
        print(f"Error: Target locale file not found: {target_file}")
        sys.exit(1)
    
    # Load English file
    try:
        with open(en_file, 'r', encoding='utf-8') as f:
            en_data = yaml.safe_load(f)
    except Exception as e:
        print(f"Error loading English file: {e}")
        sys.exit(1)
    
    # Load target locale file
    try:
        with open(target_file, 'r', encoding='utf-8') as f:
            target_data = yaml.safe_load(f)
    except Exception as e:
        print(f"Error loading target locale file: {e}")
        sys.exit(1)
    
    # Get the app section from both
    en_app = en_data.get('en-US', {}).get('app', {})
    target_app = target_data.get(target_locale, {}).get('app', {})
    
    # Flatten both
    en_flat = flatten_dict(en_app)
    target_flat = flatten_dict(target_app)
    
    # Find missing keys in target (keys in English but not in target)
    missing = []
    for key in sorted(en_flat.keys()):
        if key not in target_flat:
            missing.append(key)
    
    # Find extra keys in target (keys in target but not in English)
    extra = []
    for key in sorted(target_flat.keys()):
        if key not in en_flat:
            extra.append(key)
    
    # Report results
    print(f"Comparing {target_locale}.yml against en-US.yml")
    print(f"English keys: {len(en_flat)}")
    print(f"{target_locale} keys: {len(target_flat)}")
    print()
    
    has_errors = False
    has_warnings = False
    
    # Report missing keys (errors)
    if missing:
        has_errors = True
        print(f"✗ ERROR: Found {len(missing)} missing keys in {target_locale}.yml:")
        for key in missing:
            print(f"  - app.{key}")
        print()
    
    # Report extra keys (warnings)
    if extra:
        has_warnings = True
        print(f"⚠ WARNING: Found {len(extra)} extra keys in {target_locale}.yml (not in en-US.yml):")
        for key in extra:
            print(f"  - app.{key}")
            # Show the value to help identify what it is
            print(f"    Value: {target_flat[key]}")
        print()
    
    # Exit with appropriate code
    if has_errors:
        print("✗ Translation check FAILED: Missing keys found")
        sys.exit(1)
    elif has_warnings:
        print("⚠ Translation check PASSED with warnings: Extra keys found (may be duplicates or obsolete)")
        sys.exit(0)
    else:
        print("✓ Translation check PASSED: All keys match!")
        sys.exit(0)


if __name__ == '__main__':
    main()

