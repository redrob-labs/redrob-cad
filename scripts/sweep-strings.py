#!/usr/bin/env python3
"""Rewrites FreeCAD's product name to ours in user-visible strings of the fetched checkout, and
moves every affected translation with it.

Why the translations move too: Qt finds a translation by its SOURCE text. Rewriting a tr() string
without rewriting the .ts entry that translates it makes that string fall back to English in every
language. While this sweep was a frozen patch, 98 Korean strings did exactly that, and a running
Korean build showed every startup-loaded one of them in English.

Scope is deliberately narrow. Module names, Python import paths, the FCStd format's identifiers,
preference keys and the `FreeCAD` Python module are load-bearing names, not labels, so only these
are rewritten:

  * C++  tr() and QT_TR_NOOP() string literals. Adjacent literals are one string, as the compiler
         and lupdate both see them.
  * .ui  <string> and <title> element text, Designer's own labels.
  * .py  translate(ctx, "...") and QT_TRANSLATE_NOOP(ctx, "...") literals.
  * .ts  <source> text that one of the above rewrote, and the product name inside its translation.
         The name stays Latin in every language, Korean included.

Some strings name FreeCAD on purpose and are held back in KEEP: the project's forum, wiki, website
and donation page, its contributors, and the version history of the file format. Renaming those
would make the string false -- "an older version of Redrob CAD" for a file FreeCAD 0.19 wrote.
Licence and copyright text is never touched (KEEP_WHOLE). Submodules such as the Addon Manager are
other projects and are skipped; their strings name the FreeCAD addon ecosystem and its versions.

Bytes outside the rewritten strings are preserved, line endings included. Upstream has CRLF files,
and normalizing them turned nine of them into whole-file rewrites that conflict with any upstream
edit.

Every rewrite is listed and compared against scripts/sweep-strings.expected.tsv, the reviewed
list: an upstream bump that adds or drops a swept string fails with a diff, so the product's
wording never changes unreviewed.

Usage:
  sweep-strings.py <checkout> [--check] [--expected FILE] [--update-expected FILE] [--report FILE]

  (default)            rewrite the checkout, then prove no translation was lost
  --check              rewrite nothing; exit 1 if anything would still change
  --expected FILE      fail unless the rewrites equal FILE
  --update-expected    write the rewrites to FILE (after reviewing them)
  --report FILE        write JSON details: per-language counts and the Korean entries
"""

from __future__ import annotations

import argparse
import ast
import difflib
import html
import json
import re
import subprocess
import sys
from pathlib import Path
from xml.etree import ElementTree

URL = re.compile(r"https?://\S*")
NAME = re.compile(r"FreeCAD")
WORD = re.compile(r"[A-Za-z0-9_]")
KOREAN_NAME = "프리캐드"

# Text where "FreeCAD" names the upstream PROJECT, its file format or its version history rather
# than this application. Matched as substrings of the string's text; the name inside stays.
KEEP = (
    "FreeCAD.org",
    ".FCStd",
    "FreeCAD file",  # the format's name, which our builds still read and write
    "FreeCAD forum",
    "FreeCAD wiki",
    "FreeCAD documentation",
    "FreeCAD community",
    "FreeCAD &Forum",
    "FreeCAD W&ebsite",
    "FreeCAD website",
    "FreeCAD developers handbook",
    "FreeCAD donation",
    "Donate to FreeCA&D",
    "FreeCAD Wiki",
    "FreeCAD's GitHub",
    # The About dialog's credits list the upstream project's contributors.
    "FreeCAD would not be possible",
    # Version history of documents: a file from FreeCAD 0.19 was not written by Redrob CAD.
    "FreeCAD version",
    "FreeCAD Version",
    "version of FreeCAD",
    "versions of FreeCAD",
    "older FreeCAD",
    "Older FreeCAD",
    "newer FreeCAD",
    "FreeCAD older than",
    "made with FreeCAD",
    "improvements to FreeCAD",
    # Upstream's own theme packs, under their real names until this product ships its own.
    "FreeCAD Classic",
    "FreeCAD Dark",
    "FreeCAD Light",
    # Formats and components named after the project: .fctl/.fctb tool files, the test framework.
    "FreeCAD Tool",
    "FreeCAD UnitTest",
    "FreeCAD Unit Test",
)

# A string containing any of these is legal or attribution text, or points at upstream's own
# releases, and is left whole. Rewriting a licence statement or a copyright line is not branding.
KEEP_WHOLE = (
    "GENERAL PUBLIC LICENSE",
    "Copyright (c)",
    "github.com/FreeCAD/FreeCAD/releases",
)

# Developer examples and tools, not part of the application's interface.
SKIP_DIRS = ("src/Tools/",)

WRITE_ENCODING = dict(encoding="utf-8", errors="surrogateescape", newline="")


# --- decide which occurrences are the product name ---------------------------------------------


def flags_for(text: str) -> list[bool]:
    """For each literal 'FreeCAD' in text, in order: True if it is this application's name."""
    if any(marker in text for marker in KEEP_WHOLE):
        return [False] * len(NAME.findall(text))
    protected: list[tuple[int, int]] = [m.span() for m in URL.finditer(text)]
    for phrase in KEEP:
        start = 0
        while (i := text.find(phrase, start)) >= 0:
            protected.append((i, i + len(phrase)))
            start = i + 1
    out = []
    for m in NAME.finditer(text):
        s, e = m.span()
        before = text[s - 1] if s else ""
        after = text[e] if e < len(text) else ""
        whole_word = not (before and WORD.match(before)) and not (after and WORD.match(after))
        kept = any(a <= s and e <= b for a, b in protected)
        out.append(whole_word and not kept)
    return out


def apply_flags(raw: str, flags: list[bool], product: str) -> str:
    """Replaces the selected occurrences of 'FreeCAD' in raw (escaped) text, by position."""
    parts, last, k = [], 0, 0
    for m in NAME.finditer(raw):
        if k >= len(flags):
            raise ValueError(f"more names in the raw text than in its value: {raw[:80]!r}")
        if flags[k]:
            parts.append(raw[last : m.start()])
            parts.append(product)
            last = m.end()
        k += 1
    if k != len(flags):
        raise ValueError(f"fewer names in the raw text than in its value: {raw[:80]!r}")
    parts.append(raw[last:])
    return "".join(parts)


def rename_in_translation(raw: str, product: str) -> str:
    """The translation's own spelling of the name, Latin or Korean, becomes ours (Latin)."""
    out = re.sub(r"(?<![A-Za-z0-9_])FreeCAD(?![A-Za-z0-9_])", product, raw)
    return out.replace(KOREAN_NAME, product)


# --- read string literals as the compiler and lupdate see them ---------------------------------

CPP_ESC = {"n": "\n", "t": "\t", "r": "\r", "a": "\a", "b": "\b", "f": "\f", "v": "\v"}


def cpp_unescape(body: str) -> str:
    def one(m: re.Match) -> str:
        e = m.group(1)
        if e[0] == "x":
            return chr(int(e[1:], 16))
        if e[0] in "01234567":
            return chr(int(e, 8))
        return CPP_ESC.get(e, e)

    return re.sub(r"\\(x[0-9A-Fa-f]+|[0-7]{1,3}|.)", one, body)


CPP_CALL = re.compile(r'(?<![A-Za-z0-9_])(?:tr|QT_TR_NOOP)\(\s*(?=")')
CPP_LIT = re.compile(r'"((?:[^"\\\n]|\\.)*)"')
SPACE = re.compile(r"\s*")


def cpp_literals(text: str, pos: int) -> list[tuple[int, int]]:
    """Spans of the bodies of the adjacent literals starting at pos."""
    spans = []
    while (m := CPP_LIT.match(text, pos)) is not None:
        spans.append(m.span(1))
        pos = SPACE.match(text, m.end()).end()
    return spans


PY_CALL = re.compile(
    r"(?<![A-Za-z0-9_])(?:translate|QT_TRANSLATE_NOOP)\(\s*"
    r"(?P<cq>[\"'])(?:\\.|(?!(?P=cq)).)*(?P=cq)\s*,\s*(?=[\"'])"
)
PY_LIT = re.compile(r"(?P<q>[\"'])(?P<body>(?:\\.|(?!(?P=q))[^\\\n])*)(?P=q)")


def py_literals(text: str, pos: int) -> list[tuple[int, int, str]] | None:
    """(start, end, quote) of each adjacent literal body; None for forms we do not rewrite."""
    if text.startswith(('"""', "'''"), pos):
        return None
    spans = []
    while (m := PY_LIT.match(text, pos)) is not None:
        spans.append((m.start("body"), m.end("body"), m.group("q")))
        pos = SPACE.match(text, m.end()).end()
        if text.startswith(('"""', "'''"), pos):
            return None
    return spans


# --- the sweep -------------------------------------------------------------------------------


class Sweep:
    def __init__(self, checkout: Path, product: str, write: bool):
        self.checkout = checkout
        self.product = product
        self.write = write
        self.rows: list[tuple[str, str, str, str]] = []  # kind, path, old text, new text
        self.mapping: dict[str, list[bool]] = {}  # old translatable text -> which names change
        self.ts_counts: dict[str, int] = {}
        self.ts_mixed: list[tuple[str, str]] = []
        self.korean: list[dict] = []

    def rel(self, path: Path) -> str:
        return str(path.relative_to(self.checkout))

    def record(self, kind: str, path: Path, text: str, flags: list[bool], translatable: bool):
        new = apply_flags(text, flags, self.product)
        self.rows.append((kind, self.rel(path), text, new))
        if translatable:
            self.mapping[text] = flags

    def save(self, path: Path, original: str, rewritten: str):
        if rewritten == original:
            return
        if original.count("\r\n") != rewritten.count("\r\n"):
            raise SystemExit(f"error: line endings changed in {self.rel(path)}")
        if self.write:
            path.write_text(rewritten, **WRITE_ENCODING)

    def read(self, path: Path) -> str:
        return path.read_text(encoding="utf-8", errors="surrogateescape", newline="")

    def cpp(self, path: Path):
        text = self.read(path)
        edits = []  # (start, end, replacement)
        for call in CPP_CALL.finditer(text):
            spans = cpp_literals(text, call.end())
            bodies = [text[s:e] for s, e in spans]
            value = "".join(cpp_unescape(b) for b in bodies)
            flags = flags_for(value)
            if not any(flags):
                continue
            self.record("cpp", path, value, flags, True)
            k = 0
            for (s, e), body in zip(spans, bodies):
                n = len(NAME.findall(body))
                edits.append((s, e, apply_flags(body, flags[k : k + n], self.product)))
                k += n
        self.save(path, text, splice(text, edits))

    def ui(self, path: Path):
        text = self.read(path)
        edits = []
        for m in re.finditer(r"<(string|title)(\s[^>]*)?>([^<]*)</\1>", text):
            raw = m.group(3)
            value = html.unescape(raw)
            flags = flags_for(value)
            if not any(flags):
                continue
            translatable = 'notr="true"' not in (m.group(2) or "")
            self.record("ui", path, value, flags, translatable)
            edits.append((m.start(3), m.end(3), apply_flags(raw, flags, self.product)))
        self.save(path, text, splice(text, edits))

    def py(self, path: Path):
        text = self.read(path)
        edits = []
        for call in PY_CALL.finditer(text):
            spans = py_literals(text, call.end())
            if not spans:
                continue
            bodies = [text[s:e] for s, e, _ in spans]
            try:
                value = "".join(ast.literal_eval(q + b + q) for (_, _, q), b in zip(spans, bodies))
            except (SyntaxError, ValueError):
                continue
            flags = flags_for(value)
            if not any(flags):
                continue
            self.record("py", path, value, flags, True)
            k = 0
            for (s, e, _), body in zip(spans, bodies):
                n = len(NAME.findall(body))
                edits.append((s, e, apply_flags(body, flags[k : k + n], self.product)))
                k += n
        self.save(path, text, splice(text, edits))

    def ts(self, path: Path):
        text = self.read(path)
        lang = ts_language(path)
        edits = []
        count = 0
        for msg in re.finditer(r"<message\b[^>]*>.*?</message>", text, re.S):
            block = msg.group(0)
            src = re.search(r"<source>(.*?)</source>", block, re.S)
            if not src:
                continue
            value = html.unescape(src.group(1))
            flags = self.mapping.get(value)
            if not flags:
                continue
            new_block = (
                block[: src.start(1)]
                + apply_flags(src.group(1), flags, self.product)
                + block[src.end(1) :]
            )
            trn = re.search(r"(<translation\b[^>]*>)(.*?)(</translation>)", new_block, re.S)
            if trn and trn.group(2).strip():
                if all(flags):
                    renamed = rename_in_translation(trn.group(2), self.product)
                    new_block = new_block[: trn.start(2)] + renamed + new_block[trn.end(2) :]
                else:
                    self.ts_mixed.append((self.rel(path), value))
                if lang == "ko":
                    ctx = context_at(text, msg.start())
                    self.korean.append(
                        {
                            "file": self.rel(path),
                            "ctx": ctx,
                            "old": value,
                            "new": apply_flags(value, flags, self.product),
                            "ko": html.unescape(re.sub(r"<[^>]+>", "", trn.group(2))).strip(),
                        }
                    )
            edits.append((msg.start(), msg.end(), new_block))
            count += 1
        if count:
            self.ts_counts[lang] = self.ts_counts.get(lang, 0) + count
        self.save(path, text, splice(text, edits))

    def run(self):
        src = self.checkout / "src"
        files = sorted(
            p
            for p in src.rglob("*")
            if p.is_file() and not self.rel(p).startswith(SKIP_DIRS) and not in_submodule(p, src)
        )
        for path in files:
            if path.suffix == ".cpp":
                self.cpp(path)
            elif path.suffix == ".ui":
                self.ui(path)
            elif path.suffix == ".py":
                self.py(path)
        # Translations last: they follow the code's rewrites, never lead them.
        for path in files:
            if path.suffix == ".ts":
                self.ts(path)


_SUBMODULES: dict[Path, tuple[Path, ...]] = {}


def in_submodule(path: Path, src: Path) -> bool:
    """Submodules (AddonManager, coin, pivy, ...) are other projects, pinned by upstream. Whether
    they are checked out must not change what the sweep does, so they are always skipped."""
    checkout = src.parent
    if checkout not in _SUBMODULES:
        modules = checkout / ".gitmodules"
        paths = re.findall(r"^\s*path\s*=\s*(.+?)\s*$", modules.read_text(), re.M) if modules.is_file() else []
        _SUBMODULES[checkout] = tuple(checkout / p for p in paths)
    return any(root in path.parents for root in _SUBMODULES[checkout])


def splice(text: str, edits: list[tuple[int, int, str]]) -> str:
    out, last = [], 0
    for s, e, rep in sorted(edits):
        out.append(text[last:s])
        out.append(rep)
        last = e
    out.append(text[last:])
    return "".join(out)


def ts_language(path: Path) -> str:
    m = re.search(r"_([a-z]{2,3}(?:-[A-Za-z]+)?)\.ts$", path.name)
    return m.group(1) if m else ""


def context_at(text: str, pos: int) -> str:
    start = text.rfind("<context>", 0, pos)
    m = re.search(r"<name>(.*?)</name>", text[start:pos], re.S)
    return html.unescape(m.group(1)) if m else ""


# --- independent proof that no translation was lost ------------------------------------------


def finished(root: ElementTree.Element) -> dict[tuple[str, str], str]:
    out = {}
    for ctx in root.iter("context"):
        name = ctx.findtext("name") or ""
        for msg in ctx.iter("message"):
            source = msg.findtext("source")
            t = msg.find("translation")
            if source is None or t is None or t.get("type") in ("unfinished", "vanished", "obsolete"):
                continue
            value = "".join(t.itertext()).strip()
            if value:
                out[(name, source)] = value
    return out


def lost_translations(sweep: Sweep) -> list[str]:
    """Re-reads upstream's .ts files from git and the swept ones from disk, as XML, and checks
    that every translated string the sweep renamed is still translated under its new text."""
    lost = []
    src = sweep.checkout / "src"
    for path in sorted(src.rglob("*.ts")):
        rel = sweep.rel(path)
        if rel.startswith(SKIP_DIRS) or in_submodule(path, src):
            continue
        before = subprocess.run(
            ["git", "-C", str(sweep.checkout), "show", f"HEAD:{rel}"], capture_output=True
        ).stdout
        if not before:
            continue
        try:
            old = finished(ElementTree.fromstring(before))
            new = finished(ElementTree.parse(path).getroot())
        except ElementTree.ParseError as exc:
            lost.append(f"{rel}: does not parse after the sweep ({exc})")
            continue
        for (ctx, source), _ in old.items():
            flags = sweep.mapping.get(source)
            if not flags:
                continue
            renamed = apply_flags(source, flags, sweep.product)
            if (ctx, renamed) not in new:
                lost.append(f"{rel}: [{ctx}] {source[:70]!r}")
    return lost


# --- expected list --------------------------------------------------------------------------


def esc(s: str) -> str:
    return s.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n").replace("\r", "\\r")


def table(rows) -> list[str]:
    header = "# kind\tfile\tbefore\tafter  (generated by sweep-strings.py --update-expected; review every row)"
    return [header] + sorted("\t".join(esc(c) for c in row) for row in set(rows))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("checkout", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--expected", type=Path)
    parser.add_argument("--update-expected", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()

    branding = Path("overlay/src/Main/branding.xml")
    if not branding.is_file():
        sys.exit("error: run this from the repository root (overlay/src/Main/branding.xml not found)")
    node = ElementTree.parse(branding).getroot().find("Application")
    product = (node.text or "").strip() if node is not None else ""
    if not product:
        sys.exit("error: branding.xml declares no <Application> name")
    if not (args.checkout / "src").is_dir():
        sys.exit(f"error: {args.checkout}/src not found -- is it a fetched checkout?")

    sweep = Sweep(args.checkout.resolve(), product, write=not args.check)
    sweep.run()
    kinds = {k: sum(1 for r in sweep.rows if r[0] == k) for k in ("cpp", "ui", "py")}
    print(
        f"string sweep {'would rewrite' if args.check else 'rewrote'} {len(sweep.rows)} string(s) "
        f"(cpp {kinds['cpp']}, ui {kinds['ui']}, py {kinds['py']}) and "
        f"{sum(sweep.ts_counts.values())} translation entr(ies) in {len(sweep.ts_counts)} "
        f"language(s) -> {product!r}"
    )

    if args.check:
        if sweep.rows:
            for row in sweep.rows[:20]:
                print(f"  still names FreeCAD: {row[1]}: {row[2][:80]!r}", file=sys.stderr)
            return 1
        return 0

    if args.update_expected:
        args.update_expected.write_text("\n".join(table(sweep.rows)) + "\n", encoding="utf-8")
        print(f"wrote {args.update_expected}")

    status = 0
    if args.expected:
        want = args.expected.read_text(encoding="utf-8").splitlines()
        have = table(sweep.rows)
        if want != have:
            print(f"error: the sweep's rewrites differ from {args.expected}:", file=sys.stderr)
            for line in difflib.unified_diff(want, have, "expected", "actual", lineterm="", n=0):
                print("  " + line, file=sys.stderr)
            print("review the rows, then run with --update-expected", file=sys.stderr)
            status = 1
        else:
            print(f"rewrites match the reviewed list: {len(have) - 1} row(s)")

    if not sweep.rows:
        print("error: the sweep rewrote nothing -- an empty sweep is a broken sweep", file=sys.stderr)
        status = 1

    lost = lost_translations(sweep)
    if lost:
        print(f"error: {len(lost)} translation(s) lost by the sweep:", file=sys.stderr)
        for line in lost[:30]:
            print("  " + line, file=sys.stderr)
        status = 1
    else:
        print("no translation lost: every renamed string is still translated in every language")

    if sweep.ts_mixed:
        print(f"error: {len(sweep.ts_mixed)} translation(s) mix kept and renamed names:", file=sys.stderr)
        for rel, value in sweep.ts_mixed[:10]:
            print(f"  {rel}: {value[:80]!r}", file=sys.stderr)
        status = 1

    if args.report:
        args.report.write_text(
            json.dumps(
                {"languages": sweep.ts_counts, "korean": sweep.korean}, ensure_ascii=False, indent=1
            ),
            encoding="utf-8",
        )
    return status


if __name__ == "__main__":
    sys.exit(main())
