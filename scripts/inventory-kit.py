#!/usr/bin/env python3
"""Index source declarations and child/trait contracts in an unpacked Kit crate.

Usage: inventory-kit.py /path/to/crate/src > inventory.tsv
No network or dependency changes. This is a review index, not a parity checker:
public declarations in private modules are included, and macro-generated APIs
must also be checked at their invocation/re-export. Signatures retain source lines.
"""
import argparse
import pathlib
import re


def mask_literals(source):
    # Retain offsets/newlines while ignoring braces and declarations in comments,
    # quoted strings and raw strings. Rust char literals do not contain braces
    # except the single-character case handled here; lifetimes remain intact.
    pattern = r'//[^\n]*|/\*[\s\S]*?\*/|r(#+)?"[\s\S]*?"\1|"(?:\\.|[^"\\])*"|\'[{}]\''
    return re.sub(pattern, lambda m: re.sub(r'[^\n]', ' ', m.group()), source)


def declarations(source):
    masked = mask_literals(source)
    lines = source.splitlines()
    clean_lines = masked.splitlines()
    scopes = []
    depth = 0
    pending = ''
    pending_test = False
    test_depth = None
    for index, clean in enumerate(clean_lines):
        stripped = clean.strip()
        original = lines[index].strip()
        if stripped.startswith('#['):
            pending_test |= bool(re.search(r'cfg\s*\(\s*test\s*\)', stripped))
            continue
        enclosing_trait = any(kind == 'trait' for _, kind in scopes)
        if test_depth is None and not pending_test:
            public = re.match(r'pub\s+(?:(?:async|unsafe|const)\s+)*(?:fn|struct|enum|trait|type|mod|use|static|const)\b', stripped)
            field = re.match(r'pub\s+\w+\s*:', stripped)
            implementation = re.match(r'(?:unsafe\s+)?impl(?:\s|<)', stripped)
            trait_member = enclosing_trait and re.match(r'(?:async\s+)?(?:fn|type|const)\b', stripped)
            if public or field or implementation or trait_member:
                kind = 'impl' if implementation else 'trait-member' if trait_member and not public else 'public'
                # Include the continuation of signatures and re-export lists.
                signature = [original]
                if not field:
                    for following in range(index, len(clean_lines)):
                        current = clean_lines[following]
                        if following > index:
                            signature.append(lines[following].strip())
                        if '{' in current or ';' in current or (following == index and re.search(r'\bstruct\s+\w+\s*;', current)):
                            break
                        if following - index >= 24:
                            break
                yield index + 1, kind, ' '.join(signature)
        pending += ' ' + stripped
        for char in clean:
            if char == '{':
                depth += 1
                if pending_test and test_depth is None:
                    test_depth = depth
                pending_test = False
                kind = 'trait' if re.search(r'\bpub\s+(?:unsafe\s+)?trait\b', pending) else 'other'
                scopes.append((depth, kind))
                pending = ''
            elif char == '}':
                if test_depth == depth:
                    test_depth = None
                scopes = [(d, k) for d, k in scopes if d < depth]
                depth -= 1
                pending = ''
            elif char == ';':
                pending = ''
                pending_test = False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=pathlib.Path)
    args = parser.parse_args()
    print('source\tline\tkind\tdeclaration')
    for path in sorted(args.source.rglob('*.rs')):
        if path.stem in {'tests', 'test_support'} or path.stem.endswith('_tests'):
            continue
        for line, kind, declaration in declarations(path.read_text()):
            print(f'{path.relative_to(args.source)}\t{line}\t{kind}\t{declaration}')


if __name__ == '__main__':
    main()
