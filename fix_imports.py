#!/usr/bin/env python3
import re
from pathlib import Path

base = Path("crates/steply-core/src/widgets")

for f in sorted(base.rglob("*.rs")):
    if f.name == "traits.rs":
        continue
    content = f.read_text()
    if "impl DataNode for" not in content:
        continue

    # Check if DataNode is actually imported (in a use statement)
    has_import = bool(re.search(r'use\s+crate::widgets::traits::\{[^}]*DataNode', content))
    if has_import:
        continue

    # Add DataNode to the traits import
    new_content, count = re.subn(
        r'(use crate::widgets::traits::\{)',
        r'\1DataNode, ',
        content,
        count=1
    )
    if count == 0:
        # No grouped import found - add standalone import after last use statement
        lines = content.split('\n')
        last_use = 0
        for i, line in enumerate(lines):
            if line.startswith('use '):
                last_use = i
        lines.insert(last_use + 1, 'use crate::widgets::traits::DataNode;')
        new_content = '\n'.join(lines)

    f.write_text(new_content)
    print(f"Fixed: {f}")
