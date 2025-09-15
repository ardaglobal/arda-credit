#!/usr/bin/env python3
import os
import re
import sys

def find_markdown_links(content, file_path):
    """Find all markdown links in content and return their positions and targets"""
    links = []
    
    # Pattern for markdown links: [text](target)
    pattern = r'\[([^\]]*)\]\(([^)]+)\)'
    
    for match in re.finditer(pattern, content):
        link_text = match.group(1)
        link_target = match.group(2)
        line_number = content[:match.start()].count('\n') + 1
        links.append({
            'text': link_text,
            'target': link_target,
            'line': line_number,
            'file': file_path
        })
    
    return links

def validate_link(link, base_dir):
    """Validate if a link target exists"""
    target = link['target']
    
    # Skip external links (http/https)
    if target.startswith(('http://', 'https://')):
        return True, "external"
    
    # Skip anchor links (same file)
    if target.startswith('#'):
        return True, "anchor"
    
    # Skip email links
    if target.startswith('mailto:'):
        return True, "email"
    
    # Handle anchor links with file paths
    if '#' in target:
        target_path = target.split('#')[0]
        if not target_path:
            return True, "anchor_same_file"
    else:
        target_path = target
    
    # Convert relative path to absolute path
    if target_path.startswith('./') or target_path.startswith('../') or (not target_path.startswith('/') and not os.path.isabs(target_path)):
        # Get the directory of the file containing the link
        file_dir = os.path.dirname(link['file'])
        full_path = os.path.normpath(os.path.join(file_dir, target_path))
    else:
        full_path = os.path.join(base_dir, target_path.lstrip('/'))
    
    # Check if file exists
    if os.path.exists(full_path):
        return True, "exists"
    else:
        return False, f"missing: {full_path}"

def should_skip_file(file_path):
    """Skip files that are not part of the main documentation"""
    skip_patterns = [
        '/target/',  # Build artifacts
        '/contracts/lib/sp1-contracts/',  # External dependency
        '/node_modules/',  # Dependencies
    ]
    
    for pattern in skip_patterns:
        if pattern in file_path:
            return True
    return False

def validate_markdown_file(file_path, base_dir):
    """Validate all links in a markdown file"""
    if should_skip_file(file_path):
        return []
        
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        return [{'error': f'Cannot read file: {e}', 'file': file_path, 'line': 0}]
    
    links = find_markdown_links(content, file_path)
    results = []
    
    for link in links:
        is_valid, status = validate_link(link, base_dir)
        if not is_valid:
            results.append({
                'file': link['file'],
                'line': link['line'],
                'text': link['text'],
                'target': link['target'],
                'error': status
            })
    
    return results

def main():
    if len(sys.argv) != 2:
        print("Usage: python improved_link_validator.py <directory>")
        sys.exit(1)
    
    base_dir = sys.argv[1]
    
    # Find all markdown files
    markdown_files = []
    for root, dirs, files in os.walk(base_dir):
        for file in files:
            if file.endswith('.md'):
                markdown_files.append(os.path.join(root, file))
    
    # Filter out irrelevant files
    relevant_files = [f for f in markdown_files if not should_skip_file(f)]
    
    print(f"Found {len(relevant_files)} relevant markdown files (skipped {len(markdown_files) - len(relevant_files)} build/dependency files)")
    
    all_errors = []
    for file_path in relevant_files:
        errors = validate_markdown_file(file_path, base_dir)
        all_errors.extend(errors)
    
    if all_errors:
        print(f"\n❌ Found {len(all_errors)} broken links:")
        for error in all_errors:
            print(f"  {error['file']}:{error['line']} - [{error['text']}]({error['target']})")
            print(f"    Error: {error['error']}")
            print()
    else:
        print("\n✅ All links are valid!")

if __name__ == '__main__':
    main()
