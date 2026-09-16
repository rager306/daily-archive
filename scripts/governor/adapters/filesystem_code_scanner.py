"""Adapter: filesystem code scanner — scans source files for regex matches."""

import re
from pathlib import Path
from typing import Iterator

from governor.ports.code_scanner import CodeScanner

CFG_TEST_ATTR_RE = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")


def _cfg_test_line_ranges(lines: list[str]) -> list[tuple[int, int]]:
    """Inclusive 1-based line ranges covered by `#[cfg(test)]` items.

    Only regions opened by an explicit `#[cfg(test)]` attribute are reported, so
    production lines outside such items stay in scope. When the item body cannot
    be closed before EOF the range extends to EOF — the safe direction, because
    the only lines that can ever be skipped are the ones inside the test item.
    """
    ranges: list[tuple[int, int]] = []
    total = len(lines)
    idx = 0
    while idx < total:
        if not CFG_TEST_ATTR_RE.search(lines[idx]):
            idx += 1
            continue
        start = idx + 1
        depth = 0
        opened = False
        cursor = idx
        while cursor < total:
            for char in lines[cursor]:
                if char == "{":
                    depth += 1
                    opened = True
                elif char == "}":
                    depth -= 1
            if opened and depth <= 0:
                break
            cursor += 1
        ranges.append((start, min(cursor, total - 1) + 1))
        idx = cursor + 1
    return ranges


class FilesystemCodeScanner(CodeScanner):
    """Scans source files on the filesystem for pattern violations."""

    DEFAULT_SUFFIXES = {".py", ".json", ".md", ".toml", ".rs", ".yaml", ".yml"}

    def __init__(self, project_root: Path):
        self.root = project_root

    def scan_pattern(
        self,
        scan_path: str,
        pattern: str,
        filename_filter: str | None = None,
        path_prefix_include: str | None = None,
        path_prefix_exclude: str | None = None,
        path_contains_exclude: list[str] | None = None,
        suffix_allowlist: set[str] | None = None,
        section_filter: str | None = None,
        exclude_cfg_test_regions: bool = False,
    ) -> Iterator[tuple[Path, int, str]]:
        """Yield (file, line_number, matched_line) for each match.

        Args:
            scan_path: Directory or file to scan (relative to project root).
            pattern: Regex pattern to match.
            filename_filter: If set, only scan files with this exact name.
            path_prefix_include: If set, only scan files with this path prefix.
            path_prefix_exclude: If set, skip files with this path prefix.
            path_contains_exclude: If set, skip files whose relative path contains
                any of these substrings (e.g. ["/tests/"] for fail-closed unit tests).
            suffix_allowlist: Set of file suffixes to scan.
            section_filter: If set (e.g. "dependencies"), only match lines inside
                that TOML section header [dependencies], not [dev-dependencies].
            exclude_cfg_test_regions: If set, skip matches inside `#[cfg(test)]`
                items. Test-only code never ships, so a fail-closed unit test that
                temporarily assigns a locked value to prove rejection is not a
                production violation (see D127 rationale in adr_rules.yaml).
        """
        regex = re.compile(pattern)
        suffixes = suffix_allowlist or self.DEFAULT_SUFFIXES
        full_scan = self.root / scan_path

        if full_scan.is_file():
            files = [full_scan]
        else:
            if not full_scan.exists():
                return
            files = (p for p in full_scan.rglob("*") if p.is_file())

        for path in files:
            if filename_filter:
                if path.name != filename_filter:
                    continue
            elif path.suffix not in suffixes:
                continue

            try:
                rel = str(path.relative_to(self.root))
            except ValueError:
                continue

            if path_prefix_include and not rel.startswith(path_prefix_include):
                continue
            if path_prefix_exclude and rel.startswith(path_prefix_exclude):
                continue
            if path_contains_exclude and any(token in rel for token in path_contains_exclude):
                continue

            try:
                text = path.read_text(encoding="utf-8", errors="replace")
            except (OSError, UnicodeDecodeError):
                continue

            lines = text.splitlines()
            test_ranges = (
                _cfg_test_line_ranges(lines)
                if exclude_cfg_test_regions and path.suffix == ".rs"
                else []
            )

            current_section = None
            for line_no, line in enumerate(lines, 1):
                if test_ranges and any(lo <= line_no <= hi for lo, hi in test_ranges):
                    continue
                stripped = line.strip()
                # Track TOML/INI section headers like [dependencies] or [dev-dependencies].
                if stripped.startswith("[") and stripped.endswith("]"):
                    current_section = stripped[1:-1]
                    continue
                # If section_filter is set, skip lines outside the target section.
                if section_filter and current_section != section_filter:
                    continue
                if regex.search(line):
                    yield (path, line_no, line)
