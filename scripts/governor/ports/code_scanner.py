"""Port: code scanner — scans source files for pattern violations."""

from abc import ABC, abstractmethod
from pathlib import Path
from typing import Iterator


class CodeScanner(ABC):
    """Scans source files (Cargo.toml, .rs, .py) for rule violations."""

    @abstractmethod
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
            path_contains_exclude: If set, skip files containing any substring.
            exclude_cfg_test_regions: If set, skip matches inside `#[cfg(test)]`
                items in Rust sources (test-only code never ships).
            suffix_allowlist: Set of file suffixes to scan (e.g. {".py", ".json", ".toml"}).
        """
        ...
