"""Decide whether a pull request needs the heavy CI job.

Prints `heavy=true` or `heavy=false` (for $GITHUB_OUTPUT). Heavy is false only when every
changed file is documentation, or the diff only moves the repository's own version strings:
`[workspace.package] version` in the root Cargo.toml, the matching `version = ` entries of our
own crates in Cargo.lock, and VERSION / REDROB_VERSION / *-version.txt files. Anything else, or
anything this script cannot read, means heavy=true.

Usage: python3 .github/scripts/ci-changes.py <base-sha>
"""

import fnmatch
import glob
import re
import subprocess
import sys

DOC_GLOBS = ["*.md", "docs/*", "LICENSE*", "CHANGELOG*"]
VERSION_FILES = ["VERSION", "REDROB_VERSION", "*-version.txt"]
VERSION_LINE = re.compile(r'^version = "[0-9A-Za-z.+-]+"$')


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


def is_doc(path):
    name = path.rsplit("/", 1)[-1]
    return any(fnmatch.fnmatch(path, g) or fnmatch.fnmatch(name, g) for g in DOC_GLOBS)


def own_packages():
    names = set()
    for manifest in glob.glob("crates/*/Cargo.toml") + glob.glob("apps/*/Cargo.toml") + ["xtask/Cargo.toml"]:
        try:
            for line in open(manifest, encoding="utf-8"):
                m = re.match(r'^name = "([^"]+)"', line)
                if m:
                    names.add(m.group(1))
                    break
        except OSError:
            pass
    return names


def changed_lines(base, path, context):
    """(+/-) body lines of the diff, each with the context line just above its hunk."""
    out = []
    above = None
    for line in git("diff", f"-U{context}", base, "HEAD", "--", path).splitlines():
        if line.startswith(("diff ", "index ", "--- ", "+++ ", "@@")):
            above = None
            continue
        if line.startswith((" ",)):
            above = line[1:]
        elif line.startswith(("+", "-")):
            out.append((line[1:], above))
        # "\ No newline at end of file" is ignored
    return out


def version_only(base, path):
    if path == "Cargo.toml":
        body = changed_lines(base, path, 0)
        # Only the [workspace.package] line: the root manifest has a single top-level version.
        return bool(body) and all(VERSION_LINE.match(text) for text, _ in body)
    if path == "Cargo.lock":
        own = own_packages()
        body = changed_lines(base, path, 1)
        if not body:
            return False
        for text, above in body:
            if not VERSION_LINE.match(text):
                return False
            m = re.match(r'^name = "([^"]+)"$', above or "")
            if not m or m.group(1) not in own:
                return False
        return True
    name = path.rsplit("/", 1)[-1]
    return any(fnmatch.fnmatch(name, g) for g in VERSION_FILES)


def main():
    base = sys.argv[1]
    files = [f for f in git("diff", "--name-only", base, "HEAD").splitlines() if f]
    if not files:
        return True
    if all(is_doc(f) for f in files):
        return False
    return not all(is_doc(f) or version_only(base, f) for f in files)


if __name__ == "__main__":
    try:
        heavy = main()
    except Exception as e:  # unreadable diff: run everything
        print(f"ci-changes: {e}", file=sys.stderr)
        heavy = True
    print(f"heavy={'true' if heavy else 'false'}")
