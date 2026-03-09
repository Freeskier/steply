#!/usr/bin/env python3
"""
Migrate DataNode methods from `impl Interactive for X` / `impl OutputNode for X`
into a separate `impl DataNode for X` block.
"""

import re
import sys
from pathlib import Path

DATA_NODE_METHODS = {"value", "set_value", "on_pointer", "on_tick", "on_system_event", "task_specs", "task_subscriptions"}

def find_method_boundaries(lines, start_idx):
    """Find the end of a method starting at start_idx (the fn line)."""
    # Simple brace counting
    depth = 0
    i = start_idx
    found_open = False
    while i < len(lines):
        for ch in lines[i]:
            if ch == '{':
                depth += 1
                found_open = True
            elif ch == '}':
                depth -= 1
                if found_open and depth == 1:  # back to impl level
                    return i
        i += 1
    return i - 1

def extract_method_name(line):
    """Extract method name from a fn line."""
    m = re.match(r'\s+fn\s+(\w+)\s*\(', line)
    if m:
        return m.group(1)
    return None

def process_impl_block(lines, impl_start, impl_end, trait_name):
    """
    Given lines of an impl block, separate DataNode methods from the rest.
    Returns (remaining_methods_lines, data_node_methods_lines).
    """
    data_node_lines = []
    remaining_lines = []

    i = impl_start + 1  # skip the impl line itself
    while i < impl_end:
        line = lines[i]
        method_name = extract_method_name(line)

        if method_name and method_name in DATA_NODE_METHODS:
            # This is a DataNode method - find its end
            method_end = find_method_boundaries(lines, i)
            # Collect all lines of this method
            method_lines = lines[i:method_end + 1]
            data_node_lines.extend(method_lines)
            data_node_lines.append("")  # blank line between methods
            i = method_end + 1
            # Skip blank lines after method
            while i < impl_end and lines[i].strip() == "":
                i += 1
        else:
            remaining_lines.append(line)
            i += 1

    return remaining_lines, data_node_lines

def find_impl_blocks(content, trait_name):
    """Find all `impl TraitName for TypeName` blocks and their line ranges."""
    lines = content.split("\n")
    blocks = []
    pattern = re.compile(rf'^impl\s+{trait_name}\s+for\s+(\w+)\s*\{{')

    i = 0
    while i < len(lines):
        m = pattern.match(lines[i])
        if m:
            type_name = m.group(1)
            # Find matching closing brace
            depth = 0
            j = i
            while j < len(lines):
                for ch in lines[j]:
                    if ch == '{':
                        depth += 1
                    elif ch == '}':
                        depth -= 1
                if depth == 0:
                    blocks.append((i, j, type_name))
                    break
                j += 1
        i += 1

    return blocks, lines

def process_file(filepath):
    content = filepath.read_text()
    lines = content.split("\n")

    # Find Interactive and OutputNode impl blocks
    changes = []

    for trait_name in ["Interactive", "OutputNode"]:
        blocks, _ = find_impl_blocks(content, trait_name)
        for (start, end, type_name) in blocks:
            # Check if any DataNode methods exist in this block
            has_data_node_methods = False
            for i in range(start, end + 1):
                method_name = extract_method_name(lines[i])
                if method_name and method_name in DATA_NODE_METHODS:
                    has_data_node_methods = True
                    break

            if has_data_node_methods:
                changes.append((start, end, type_name, trait_name))

    if not changes:
        return False

    # Process from bottom to top to preserve line numbers
    changes.sort(key=lambda x: x[0], reverse=True)

    for (start, end, type_name, trait_name) in changes:
        # Extract the impl block lines
        impl_header = lines[start]
        impl_body_lines = lines[start+1:end]

        # Separate DataNode methods
        data_node_methods = []
        remaining_body = []

        i = 0
        while i < len(impl_body_lines):
            line = impl_body_lines[i]
            method_name = extract_method_name(line)

            if method_name and method_name in DATA_NODE_METHODS:
                # Find method end by brace counting
                depth = 0
                j = i
                found_open = False
                while j < len(impl_body_lines):
                    for ch in impl_body_lines[j]:
                        if ch == '{':
                            depth += 1
                            found_open = True
                        elif ch == '}':
                            depth -= 1
                    if found_open and depth == 0:
                        break
                    # One-line methods like `fn set_value(&mut self, _value: Value) {}`
                    if found_open and depth == 0:
                        break
                    j += 1

                # Check for one-liner (e.g., `fn foo() {}` on single line)
                if not found_open:
                    # single line without braces? shouldn't happen
                    data_node_methods.append(line)
                    i += 1
                    continue

                data_node_methods.extend(impl_body_lines[i:j+1])
                data_node_methods.append("")
                i = j + 1
                # Skip trailing blank lines
                while i < len(impl_body_lines) and impl_body_lines[i].strip() == "":
                    i += 1
            else:
                remaining_body.append(line)
                i += 1

        # Clean up trailing empty lines in data_node_methods
        while data_node_methods and data_node_methods[-1].strip() == "":
            data_node_methods.pop()

        # Clean up trailing empty lines in remaining_body
        while remaining_body and remaining_body[-1].strip() == "":
            remaining_body.pop()

        # Build new blocks
        new_lines = []

        # DataNode impl first
        new_lines.append(f"impl DataNode for {type_name} {{")
        new_lines.extend(data_node_methods)
        new_lines.append("}")
        new_lines.append("")

        # Then original trait impl with remaining methods
        new_lines.append(impl_header)
        new_lines.extend(remaining_body)
        new_lines.append("}")

        # Replace in lines array
        lines[start:end+1] = new_lines

    # Add DataNode import if needed
    new_content = "\n".join(lines)

    # Check if DataNode is already imported
    if "DataNode" not in new_content:
        # Find the traits import line and add DataNode
        new_content = re.sub(
            r'(use crate::widgets::traits::\{[^}]*)\b(Interactive|OutputNode)\b',
            lambda m: m.group(0) if 'DataNode' in m.group(1) else m.group(1) + 'DataNode, ' + m.group(2),
            new_content,
            count=1
        )

    filepath.write_text(new_content)
    return True

def main():
    base = Path("/home/freeskier/projects/steply/crates/steply-core/src/widgets")

    # Get all .rs files
    files = list(base.rglob("*.rs"))

    modified = 0
    for f in sorted(files):
        if f.name == "traits.rs":
            continue
        content = f.read_text()
        if "impl Interactive for" not in content and "impl OutputNode for" not in content:
            continue

        # Check if any DataNode method is defined in Interactive/OutputNode impl
        has_data_method = False
        for method in DATA_NODE_METHODS:
            if re.search(rf'fn\s+{method}\s*\(', content):
                # Check it's inside an Interactive or OutputNode impl
                has_data_method = True
                break

        if not has_data_method:
            continue

        if process_file(f):
            modified += 1
            print(f"  Modified: {f.relative_to(base)}")

    print(f"\nModified {modified} files")

if __name__ == "__main__":
    main()
