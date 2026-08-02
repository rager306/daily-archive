"""Adapter: filesystem code scanner — scans source files for regex matches."""

import re
from pathlib import Path
from typing import Iterator

from governor.ports.code_scanner import CodeScanner


class FilesystemCodeScanner(CodeScanner):
    """Scans source files on the filesystem for pattern violations."""

    DEFAULT_SUFFIXES = {".py", ".json", ".md", ".toml", ".rs"}

    def __init__(self, project_root: Path):
        self.root = project_root

    def scan_pattern(
        self,
        scan_path: str,
        pattern: str,
        filename_filter: str | None = None,
        path_prefix_include: str | None = None,
        path_prefix_exclude: str | None = None,
        suffix_allowlist: set[str] | None = None,
        section_filter: str | None = None,
    ) -> Iterator[tuple[Path, int, str]]:
        """Yield (file, line_number, matched_line) for each match.

        Args:
            scan_path: Directory or file to scan (relative to project root).
            pattern: Regex pattern to match.
            filename_filter: If set, only scan files with this exact name.
            path_prefix_include: If set, only scan files with this path prefix.
            path_prefix_exclude: If set, skip files with this path prefix.
            suffix_allowlist: Set of file suffixes to scan.
            section_filter: If set (e.g. "dependencies"), only match lines inside
                that TOML section header [dependencies], not [dev-dependencies].
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

            try:
                text = path.read_text(encoding="utf-8", errors="replace")
            except (OSError, UnicodeDecodeError):
                continue

            current_section = None
            for line_no, line in enumerate(text.splitlines(), 1):
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
