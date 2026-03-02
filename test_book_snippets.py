#!/usr/bin/env python3
"""Extract ```iris code blocks from BOOK.md and compile-check each one."""

import os
import re
import subprocess
import sys
import tempfile

BOOK_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "BOOK.md")
# Prefer release binary for speed; fall back to debug
IRIS_EXE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "target", "release", "iris.exe")
if not os.path.isfile(IRIS_EXE):
    IRIS_EXE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "target", "debug", "iris.exe")

PREVIEW_LINES = 4  # how many lines of each snippet to show


def is_expected_failure(code: str) -> bool:
    """Return True if a snippet is expected to fail — shows wrong code,
    uses bring (needs external module), or is a type-only illustration."""
    first_lines = code.strip().split('\n')[:3]
    first_text = '\n'.join(first_lines).lower()
    # Snippets showing intentional errors
    if '// wrong' in first_text or '// likely wrong' in first_text:
        return True
    # Snippets that bring external modules
    if code.strip().startswith('bring '):
        return True
    # Snippets using // src/ path comments (multi-file examples)
    if '// src/' in first_text:
        return True
    return False


def extract_iris_blocks(text: str):
    """Yield (line_number, code) for every ```iris fenced block."""
    pattern = re.compile(r"^```iris\b[^\n]*\n(.*?)^```", re.MULTILINE | re.DOTALL)
    for m in pattern.finditer(text):
        # line number of the opening ``` (1-based)
        line_no = text[:m.start()].count("\n") + 1
        yield line_no, m.group(1)


def preview(code: str, n: int = PREVIEW_LINES) -> str:
    lines = code.splitlines()
    preview_lines = lines[:n]
    text = "\n".join(f"    {l}" for l in preview_lines)
    if len(lines) > n:
        text += f"\n    ... ({len(lines) - n} more lines)"
    # Replace non-ASCII chars that Windows cp1252 can't handle
    return text.encode("ascii", "replace").decode("ascii")


def main():
    if not os.path.isfile(BOOK_PATH):
        print(f"ERROR: {BOOK_PATH} not found.")
        sys.exit(1)
    if not os.path.isfile(IRIS_EXE):
        print(f"ERROR: {IRIS_EXE} not found. Build with `cargo build` first.")
        sys.exit(1)

    with open(BOOK_PATH, encoding="utf-8") as f:
        book_text = f.read()

    blocks = list(extract_iris_blocks(book_text))
    if not blocks:
        print("No ```iris code blocks found in BOOK.md.")
        sys.exit(0)

    passed = []
    failed = []
    xfailed = []   # expected failures (intentional bad examples, bring, etc.)
    xpassed = []   # expected failures that unexpectedly passed

    print(f"Found {len(blocks)} iris snippet(s) in BOOK.md\n")
    print("=" * 70)

    for idx, (line_no, code) in enumerate(blocks, 1):
        expected_fail = is_expected_failure(code)

        # Write snippet to a temp file
        tmp = tempfile.NamedTemporaryFile(
            mode="w", suffix=".iris", delete=False, encoding="utf-8"
        )
        try:
            tmp.write(code)
            tmp.close()

            result = subprocess.run(
                [IRIS_EXE, "--emit", "ir", tmp.name],
                capture_output=True,
                text=True,
                timeout=30,
            )
            ok = result.returncode == 0
        except subprocess.TimeoutExpired:
            ok = False
            result = None
        finally:
            os.unlink(tmp.name)

        # Classify result
        if expected_fail:
            if ok:
                status = "XPASS"
                marker = "?"
                xpassed.append((idx, line_no))
            else:
                status = "XFAIL"
                marker = "~"
                xfailed.append((idx, line_no))
        elif ok:
            status = "PASS"
            marker = "+"
            passed.append((idx, line_no))
        else:
            status = "FAIL"
            marker = "X"
            failed.append((idx, line_no, code, result))

        print(f"\n[{marker}] Snippet #{idx}  (BOOK.md line {line_no})  {status}")
        print(preview(code))

        if not ok and result is not None:
            err = (result.stderr or result.stdout or "").strip()
            if err:
                # Show first few lines of compiler output
                err_lines = err.splitlines()[:6]
                print("  Error:")
                for el in err_lines:
                    print(f"    {el}")
                if len(err.splitlines()) > 6:
                    print(f"    ... ({len(err.splitlines()) - 6} more lines)")

    # Summary
    print("\n" + "=" * 70)
    parts = [
        f"{len(blocks)} total",
        f"{len(passed)} passed",
        f"{len(failed)} failed",
    ]
    if xfailed:
        parts.append(f"{len(xfailed)} expected-fail")
    if xpassed:
        parts.append(f"{len(xpassed)} unexpected-pass")
    print(f"\nSummary:  {',  '.join(parts)}\n")

    if failed:
        print("Failed snippets:")
        for idx, line_no, _code, _result in failed:
            print(f"  - Snippet #{idx}  (line {line_no})")

    if xpassed:
        print("Unexpected passes (remove from expected-failure list?):")
        for idx, line_no in xpassed:
            print(f"  - Snippet #{idx}  (line {line_no})")

    sys.exit(0 if not failed else 1)


if __name__ == "__main__":
    main()
