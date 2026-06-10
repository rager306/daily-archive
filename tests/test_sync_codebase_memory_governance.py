"""Tests for scripts/sync_codebase_memory_governance.py.

Ensures the codebase-memory MCP mirror stays in sync with the
canonical ADR sources. Per D075 (GSD remains canonical) and
D076 (typed governance graph projection), the mirror must reflect
ALL ADRs — both M034 package and project-level.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SYNC_SCRIPT = REPO_ROOT / "scripts" / "sync_codebase_memory_governance.py"
ADR_DIR = REPO_ROOT / "doc" / "adr"
ADR_MIRROR = REPO_ROOT / ".codebase-memory" / "adr.md"
GRAPH_MIRROR = REPO_ROOT / ".codebase-memory" / "governance-graph.json"


def _run_sync() -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SYNC_SCRIPT)],
        capture_output=True,
        text=True,
        check=False,
    )


def _all_adr_files() -> list[Path]:
    """All canonical ADR files (project + M034)."""
    return [
        p
        for p in ADR_DIR.rglob("ADR-*.md")
        if p.name.startswith("ADR-")
        and "TEMPLATE" not in p.name
        and "INDEX" not in p.name
    ]


def test_sync_script_runs_clean() -> None:
    result = _run_sync()
    assert result.returncode == 0, result.stderr
    assert ADR_MIRROR.exists()
    assert GRAPH_MIRROR.exists()


def test_mirror_includes_all_canonical_adrs() -> None:
    """Each canonical ADR file must appear in the mirror's ADR list."""
    _run_sync()
    mirror_text = ADR_MIRROR.read_text(encoding="utf-8")
    canonical_ids: set[str] = set()
    for adr_file in _all_adr_files():
        text = adr_file.read_text(encoding="utf-8")
        match = re.search(r"^#\s+(ADR-\d+):\s+(.+)$", text, flags=re.MULTILINE)
        if match:
            canonical_ids.add(match.group(1))
    # The mirror must mention each canonical ADR id
    for adr_id in sorted(canonical_ids):
        assert f"| {adr_id} |" in mirror_text, f"ADR {adr_id} missing from mirror"


def test_mirror_includes_project_level_adrs() -> None:
    """Project-level ADRs at doc/adr/ (not in m034/) must be in mirror."""
    _run_sync()
    mirror_text = ADR_MIRROR.read_text(encoding="utf-8")
    # ADR-001 and ADR-008 are project-level (M046 QW-1 and M055 S05)
    assert "ADR-001" in mirror_text, "ADR-001 (project-level) missing from mirror"
    assert "ADR-008" in mirror_text, "ADR-008 (project-level) missing from mirror"
    # Verify they point to project-level paths, not m034
    assert "doc/adr/ADR-001-" in mirror_text
    assert "doc/adr/ADR-008-" in mirror_text


def test_mirror_includes_m034_historical_adrs() -> None:
    """M034 package ADRs (000-007) must remain in mirror."""
    _run_sync()
    mirror_text = ADR_MIRROR.read_text(encoding="utf-8")
    for adr_id in ["ADR-000", "ADR-002", "ADR-003", "ADR-004", "ADR-005", "ADR-006", "ADR-007"]:
        assert f"| {adr_id} |" in mirror_text, f"{adr_id} (M034) missing from mirror"


def test_governance_graph_includes_canonical_sources() -> None:
    """The governance graph should reference canonical sources."""
    _run_sync()
    graph = json.loads(GRAPH_MIRROR.read_text(encoding="utf-8"))
    assert "canonical_sources" in graph
    assert "adrs" in graph["canonical_sources"]


def test_mirror_d_numbers_match_canonical() -> None:
    """D-numbers in mirror must match .gsd/DECISIONS.md canonical register."""
    _run_sync()
    canonical = (REPO_ROOT / ".gsd" / "DECISIONS.md").read_text(encoding="utf-8")
    mirror = ADR_MIRROR.read_text(encoding="utf-8")
    canonical_d = set(re.findall(r"D\d{3}", canonical))
    mirror_d = set(re.findall(r"D\d{3}", mirror))
    missing = canonical_d - mirror_d
    assert not missing, f"Mirror missing D-numbers: {sorted(missing)}"
