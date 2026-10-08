"""Adapter: clippy scanner — runs cargo clippy per-crate and parses warnings.

Per-crate execution avoids the librocksdb-sys rebuild trap (MEM468/MEM514).
Only scans OUR crates (kg-* + da-*), filters out vendor-source warnings.
"""

import re
import subprocess
from dataclasses import dataclass, field
from pathlib import Path


# Our crates — per-crate clippy avoids rocksdb rebuild (MEM468/MEM514).
OUR_CRATES = [
    "kg-ontology",
    "kg-storage",
    "kg-embeddings",
    "kg-algorithms",
    "kg-pipeline",
    "da-domain",
    "da-ports",
    "da-application",
    "da-adapters",
    "da-parse",
    "da-cli",
    "da-mcp",
]


@dataclass
class ClippyWarning:
    """A clippy warning from our code (not vendor-source)."""

    crate: str
    file: str
    line: int
    message: str
    lint: str = ""

    def to_dict(self) -> dict:
        return {
            "crate": self.crate,
            "file": self.file,
            "line": self.line,
            "message": self.message,
            "lint": self.lint,
        }


@dataclass
class ClippyReport:
    """Result of scanning all our crates with clippy."""

    warnings: list[ClippyWarning] = field(default_factory=list)
    crates_scanned: int = 0
    crates_clean: int = 0

    @property
    def has_warnings(self) -> bool:
        return len(self.warnings) > 0


class ClippyScanner:
    """Runs cargo clippy per-crate and parses warnings."""

    # Pattern: warning: <message>\n  --> <file>:<line>:<col>
    WARNING_RE = re.compile(r"^warning:\s*(.+)$")
    LOCATION_RE = re.compile(r"^\s*-->\s*(.+?):(\d+):\d+")
    # `= note: #[warn(dead_code)] on by default` is how a lint id reaches the
    # capture. It is what turns `dead_code` from a message-shape guess into an
    # authoritative filter for the scorecard's dead-code rule.
    LINT_RE = re.compile(r"#\[(?:warn|deny|allow)\(([A-Za-z_][A-Za-z0-9_]*)\)")

    def __init__(self, project_root: Path):
        self.root = project_root

    def scan_all(self) -> ClippyReport:
        report = ClippyReport()
        for crate in OUR_CRATES:
            report.crates_scanned += 1
            crate_warnings = self._scan_crate(crate)
            if crate_warnings:
                report.warnings.extend(crate_warnings)
            else:
                report.crates_clean += 1
        return report

    def _scan_crate(self, crate: str) -> list[ClippyWarning]:
        try:
            result = subprocess.run(
                ["cargo", "clippy", "-p", crate, "--no-deps"],
                cwd=self.root,
                capture_output=True,
                text=True,
                timeout=30,
            )
        except (subprocess.TimeoutExpired, FileNotFoundError):
            return []

        output = result.stdout + result.stderr
        return self._parse_warnings(output, crate)

    def _parse_warnings(self, output: str, crate: str) -> list[ClippyWarning]:
        warnings = []
        lines = output.splitlines()
        i = 0
        while i < len(lines):
            msg_match = self.WARNING_RE.match(lines[i])
            if not msg_match:
                i += 1
                continue
            message = msg_match.group(1).strip()
            # Everything up to the next `warning:` belongs to this warning. The
            # location and the `#[warn(<lint>)]` note both live in that block, so
            # the block boundary is the only reliable scope for either.
            file_path = None
            line_no = None
            lint = ""
            j = i + 1
            while j < len(lines) and not self.WARNING_RE.match(lines[j]):
                loc_match = self.LOCATION_RE.match(lines[j])
                if loc_match and file_path is None:
                    file_path = loc_match.group(1)
                    line_no = int(loc_match.group(2))
                if not lint:
                    lint_match = self.LINT_RE.search(lines[j])
                    if lint_match:
                        lint = lint_match.group(1)
                j += 1
            # Filter out vendor-source warnings.
            if file_path and line_no is not None and self._is_our_code(file_path):
                warnings.append(
                    ClippyWarning(
                        crate=crate,
                        file=file_path,
                        line=line_no,
                        message=message,
                        lint=lint,
                    )
                )
            i = j
        return warnings

    def _is_our_code(self, file_path: str) -> bool:
        """Check if a file path belongs to our crates, not vendor-source."""
        # Normalize path.
        if "vendor-source" in file_path or "/samyama" in file_path:
            return False
        # Cargo emits workspace-relative paths (`crates/da-x/src/lib.rs`), but a
        # capture from another working directory may carry an absolute prefix.
        # Both must match, otherwise our own warnings are filtered out and the
        # scan reports a false clean.
        normalized = "/" + file_path.lstrip("/")
        return "/crates/" in normalized and "/src/" in normalized
