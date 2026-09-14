#!/usr/bin/env python3
"""Rewrites FreeCAD's product name to ours inside user-visible strings in the fetched checkout.

Scope is deliberately narrow, and the narrowness is the point. This repository's README commits to
preserving "the complete FreeCAD C++/Python runtime, workbench ecosystem, file compatibility". A
blanket rename would break all three: module names, Python import paths, the FCStd file format's
own identifiers, preference-group keys and the `FreeCAD` Python module itself are load-bearing
names, not labels.

So only two things are rewritten:

  * Qt `tr("...")` arguments in C++ -- the strings Qt marks for translation, which is exactly the
    set that reaches a user's screen.
  * `<string>` and `<title>` element text in `.ui` files -- Designer's own labels.

Everything else keeps FreeCAD's naming: identifiers, includes, file extensions, preference keys,
Python modules, URLs, and any string not marked translatable.

Usage: sweep-strings.py <checkout-root> [--check]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from xml.etree import ElementTree

# Whole word only. "FreeCADGui", "FreeCADCmd" and similar are module and executable names.
NAME = re.compile(r"\bFreeCAD\b(?!Gui|Cmd|Base|App|Guy)")

# A tr(" ... ") argument. Qt also has QT_TR_NOOP and tr() with a disambiguation second argument; the
# first string is the visible one either way, which is what this captures.
TR = re.compile(r'(tr\(\s*")((?:[^"\\]|\\.)*)(")')

URL = re.compile(r"https?://\S*")

# Text that is a name rather than a label, even inside a translatable string.
KEEP = (
    "FreeCAD.org",
    ".FCStd",
    "FreeCAD file",       # the format's name, which our builds still read and write
    "FreeCAD forum",
    "FreeCAD wiki",
    "FreeCAD documentation",
    "FreeCAD community",
)


def rewrite(text: str, product: str) -> str:
    """Rewrites the product name, leaving URLs and format/community references alone."""
    stash: list[str] = []

    def hide(value: str) -> str:
        stash.append(value)
        return f"\x00{len(stash) - 1}\x00"

    # URLs first: freecad.org links stay valid links.
    body = URL.sub(lambda m: hide(m.group(0)), text)

    # Then phrases where "FreeCAD" is a name rather than a label -- the file format, the project's
    # own forum and wiki. Renaming those would make the string false: our builds read FreeCAD files
    # and the forum is not ours.
    for phrase in KEEP:
        if phrase in body:
            body = body.replace(phrase, hide(phrase))

    body = NAME.sub(product, body)

    return re.sub(r"\x00(\d+)\x00", lambda m: stash[int(m.group(1))], body)


def sweep_cpp(path: Path, product: str) -> tuple[str, int]:
    text = path.read_text(encoding="utf-8", errors="surrogateescape")
    changes = 0

    def on_tr(match: re.Match) -> str:
        nonlocal changes
        original = match.group(2)
        rewritten = rewrite(original, product)
        if rewritten != original:
            changes += 1
        return match.group(1) + rewritten + match.group(3)

    return TR.sub(on_tr, text), changes


def sweep_ui(path: Path, product: str) -> tuple[str, int]:
    """Rewrites <string> and <title> text, editing the file as text to keep formatting intact."""
    text = path.read_text(encoding="utf-8", errors="surrogateescape")
    changes = 0

    def on_element(match: re.Match) -> str:
        nonlocal changes
        open_tag, body, close_tag = match.group(1), match.group(2), match.group(3)
        rewritten = rewrite(body, product)
        if rewritten != body:
            changes += 1
        return open_tag + rewritten + close_tag

    pattern = re.compile(r"(<(?:string|title)(?:\s[^>]*)?>)([^<]*)(</(?:string|title)>)")
    return pattern.sub(on_element, text), changes


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    overlay = Path("overlay/src/Main/branding.xml")
    if not overlay.is_file():
        sys.exit("error: run this from the repository root (overlay/src/Main/branding.xml not found)")
    branding = ElementTree.parse(overlay).getroot()
    node = branding.find("Application")
    product = (node.text or "").strip() if node is not None else ""
    if not product:
        sys.exit("error: branding.xml declares no <Application> name")

    src = args.root / "src"
    if not src.is_dir():
        sys.exit(f"error: {src} not found -- is {args.root} a fetched checkout?")

    files = 0
    total = 0
    for path in sorted(src.rglob("*")):
        if path.suffix == ".cpp":
            rewritten, changes = sweep_cpp(path, product)
        elif path.suffix == ".ui":
            rewritten, changes = sweep_ui(path, product)
        else:
            continue
        if changes == 0:
            continue
        files += 1
        total += changes
        if not args.check:
            path.write_text(rewritten, encoding="utf-8", errors="surrogateescape")

    verb = "would rewrite" if args.check else "rewrote"
    print(f"string sweep {verb} {total} user-visible string(s) in {files} file(s) -> {product!r}")

    if args.check and total:
        print("error: user-visible strings still name FreeCAD; run without --check and regenerate "
              "the patch", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
