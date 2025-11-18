#!/usr/bin/env python3
"""Convert Mermaid diagram to SVG using mermaid.ink API."""

import urllib.parse
import urllib.request
import json
import sys
import os

def extract_mermaid_code(md_file):
    """Extract Mermaid code from markdown file."""
    with open(md_file, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Find the mermaid code block
    start_marker = '```mermaid\n'
    end_marker = '\n```'
    
    start_idx = content.find(start_marker)
    if start_idx == -1:
        raise ValueError("Mermaid code block not found")
    
    start_idx += len(start_marker)
    end_idx = content.find(end_marker, start_idx)
    if end_idx == -1:
        raise ValueError("Mermaid code block not properly closed")
    
    return content[start_idx:end_idx]

def mermaid_to_svg(mermaid_code):
    """Convert Mermaid code to SVG using kroki.io API."""
    import base64
    import zlib
    import ssl
    
    # Create SSL context that doesn't verify certificates (for corporate proxies)
    ssl_context = ssl.create_default_context()
    ssl_context.check_hostname = False
    ssl_context.verify_mode = ssl.CERT_NONE
    
    # Try kroki.io API (more reliable for large diagrams)
    # kroki.io uses base64url encoding of compressed mermaid code
    compressed = zlib.compress(mermaid_code.encode('utf-8'), level=9)
    encoded = base64.urlsafe_b64encode(compressed).decode('ascii').rstrip('=')
    
    url = f"https://kroki.io/mermaid/svg/{encoded}"
    
    try:
        req = urllib.request.Request(url)
        with urllib.request.urlopen(req, timeout=60, context=ssl_context) as response:
            svg_content = response.read().decode('utf-8')
            return svg_content
    except urllib.error.HTTPError as e:
        # Fallback to mermaid.ink with POST
        try:
            print("Trying mermaid.ink API as fallback...")
            api_url = "https://mermaid.ink/api/v1/svg"
            data = json.dumps({"code": mermaid_code}).encode('utf-8')
            req = urllib.request.Request(api_url, data=data, headers={'Content-Type': 'application/json'})
            with urllib.request.urlopen(req, timeout=60, context=ssl_context) as response:
                svg_content = response.read().decode('utf-8')
                return svg_content
        except Exception as e2:
            raise Exception(f"Both APIs failed. kroki.io error: {e.code}: {e.reason}, mermaid.ink error: {e2}")

def main():
    md_file = "ControlFlow_Diagram.md"
    svg_file = "ControlFlow_Diagram.svg"
    
    if not os.path.exists(md_file):
        print(f"Error: {md_file} not found")
        sys.exit(1)
    
    print(f"Extracting Mermaid code from {md_file}...")
    mermaid_code = extract_mermaid_code(md_file)
    
    print(f"Converting to SVG using mermaid.ink API...")
    svg_content = mermaid_to_svg(mermaid_code)
    
    print(f"Writing SVG to {svg_file}...")
    with open(svg_file, 'w', encoding='utf-8') as f:
        f.write(svg_content)
    
    print(f"Successfully created {svg_file}")

if __name__ == "__main__":
    main()

