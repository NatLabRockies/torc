#!/usr/bin/env python3
"""Check documentation links.

The internal checker validates Markdown links to local files and headings under docs/src plus
docs/README.md. The external checker validates HTTP(S) URLs that appear outside fenced code blocks
and inline code spans.
"""

from __future__ import annotations

import argparse
import re
import sys
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DOCS_ROOT = REPO_ROOT / "docs"
DOCS_SRC = DOCS_ROOT / "src"
LOCALHOST_NAMES = ("localhost", "127.0.0.1", "::1")


@dataclass(frozen=True)
class LinkProblem:
    path: Path
    line: int
    target: str
    kind: str
    detail: str


def docs_files() -> list[Path]:
    return sorted(DOCS_SRC.rglob("*.md")) + [DOCS_ROOT / "README.md"]


def non_code_lines(text: str, *, strip_inline_code: bool) -> list[tuple[int, str]]:
    lines: list[tuple[int, str]] = []
    in_fence = False
    for line_no, line in enumerate(text.splitlines(), 1):
        if re.match(r"^\s*```", line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if strip_inline_code:
            line = re.sub(r"`[^`]*`", "", line)
        lines.append((line_no, line))
    return lines


def mdbook_slug(heading: str) -> str:
    heading = re.sub(r"<[^>]+>", "", heading)
    heading = re.sub(r"`([^`]*)`", r"\1", heading)
    heading = re.sub(r"\[([^\]]+)\]\([^\)]*\)", r"\1", heading)
    heading = heading.strip().lower()
    heading = "".join(
        char
        for char in unicodedata.normalize("NFKD", heading)
        if not unicodedata.combining(char)
    )
    heading = re.sub(r"[^\w\s-]", "", heading, flags=re.UNICODE)
    return re.sub(r"\s+", "-", heading)


def collect_anchors(files: list[Path]) -> dict[Path, set[str]]:
    anchors: dict[Path, set[str]] = {}
    for path in files:
        counts: dict[str, int] = {}
        file_anchors = {""}
        for _, line in non_code_lines(path.read_text(errors="replace"), strip_inline_code=False):
            match = re.match(r"^(#{1,6})\s+(.*?)\s*#*\s*$", line)
            if not match:
                continue
            base = mdbook_slug(match.group(2))
            count = counts.get(base, 0)
            counts[base] = count + 1
            file_anchors.add(base if count == 0 else f"{base}-{count}")
        anchors[path.resolve()] = file_anchors
    return anchors


def check_internal_links(files: list[Path]) -> list[LinkProblem]:
    anchors = collect_anchors(files)
    problems: list[LinkProblem] = []
    link_re = re.compile(r'(?<!!)\[[^\]]+\]\(([^)\s]+)(?:\s+"[^"]*")?\)')

    for path in files:
        for line_no, line in non_code_lines(
            path.read_text(errors="replace"), strip_inline_code=True
        ):
            for match in link_re.finditer(line):
                raw = match.group(1).strip("<>")
                if not raw or raw.startswith(("http://", "https://", "mailto:", "tel:")):
                    continue

                if raw.startswith("#"):
                    target = path.resolve()
                    fragment = raw[1:]
                else:
                    raw_path, _, fragment = raw.partition("#")
                    raw_path = urllib.parse.unquote(raw_path)
                    if raw_path.startswith("/"):
                        target = (DOCS_SRC / raw_path.lstrip("/")).resolve()
                    else:
                        target = (path.parent / raw_path).resolve()

                if not target.exists():
                    problems.append(
                        LinkProblem(path, line_no, raw, "missing file", display_path(target))
                    )
                elif fragment:
                    fragment = urllib.parse.unquote(fragment)
                    if fragment not in anchors.get(target, {""}):
                        problems.append(
                            LinkProblem(path, line_no, raw, "missing anchor", fragment)
                        )

    return problems


def collect_external_urls(files: list[Path]) -> list[tuple[Path, int, str]]:
    urls: list[tuple[Path, int, str]] = []
    seen: set[str] = set()
    url_re = re.compile(r'https?://[^\s)>"\]]+')

    for path in files:
        for line_no, line in non_code_lines(
            path.read_text(errors="replace"), strip_inline_code=True
        ):
            for match in url_re.finditer(line):
                url = match.group(0).rstrip(".,;:")
                host = urllib.parse.urlparse(url).hostname or ""
                if host in LOCALHOST_NAMES:
                    continue
                if url in seen:
                    continue
                seen.add(url)
                urls.append((path, line_no, url))

    return urls


def open_url(url: str, method: str, timeout: int) -> urllib.response.addinfourl:
    request = urllib.request.Request(
        url,
        method=method,
        headers={"User-Agent": "Mozilla/5.0 (compatible; torc-doc-link-check/1.0)"},
    )
    return urllib.request.urlopen(request, timeout=timeout)


def check_external_url(url: str, timeout: int, retries: int) -> str | None:
    last_error = ""
    for attempt in range(retries + 1):
        for method in ("HEAD", "GET"):
            try:
                with open_url(url, method, timeout):
                    return None
            except urllib.error.HTTPError as err:
                last_error = f"HTTP {err.code}"
                if method == "HEAD":
                    continue
            except Exception as err:  # noqa: BLE001 - diagnostics should preserve the source error.
                last_error = f"{type(err).__name__}: {str(err)[:120]}"
                if method == "HEAD":
                    continue

        if attempt < retries:
            time.sleep(1 + attempt)

    return last_error


def check_external_links(files: list[Path], timeout: int, retries: int) -> list[LinkProblem]:
    problems: list[LinkProblem] = []
    for path, line_no, url in collect_external_urls(files):
        error = check_external_url(url, timeout, retries)
        if error:
            problems.append(LinkProblem(path, line_no, url, "external link failed", error))
    return problems


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def print_problems(problems: list[LinkProblem]) -> None:
    for problem in problems:
        location = f"{display_path(problem.path)}:{problem.line}"
        print(f"{location}: {problem.kind}: {problem.target} ({problem.detail})")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--internal", action="store_true", help="check local file and anchor links")
    parser.add_argument("--external", action="store_true", help="check HTTP(S) links")
    parser.add_argument("--timeout", type=int, default=12, help="external URL timeout in seconds")
    parser.add_argument("--retries", type=int, default=2, help="external URL retry count")
    args = parser.parse_args()

    if not args.internal and not args.external:
        args.internal = True
        args.external = True

    files = docs_files()
    problems: list[LinkProblem] = []
    if args.internal:
        problems.extend(check_internal_links(files))
    if args.external:
        problems.extend(check_external_links(files, args.timeout, args.retries))

    if problems:
        print_problems(problems)
        return 1

    checks = []
    if args.internal:
        checks.append("internal")
    if args.external:
        checks.append("external")
    print(f"Documentation link check passed ({', '.join(checks)}).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
