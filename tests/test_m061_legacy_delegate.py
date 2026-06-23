"""Tests for scripts/m061_ingest_to_canonical_catalog.py legacy delegate.

Verifies that the deprecated M061 script:
1. Emits DeprecationWarning at import time
2. Forwards args to scripts/ingest_to_canonical_catalog.py
3. Produces identical behavior to new CLI (32 skipped in catalog-already-populated state)
"""

from __future__ import annotations

import subprocess
import sys
import warnings
from pathlib import Path

import pytest

LEGACY_CLI = Path(__file__).resolve().parents[1] / "scripts" / "m061_ingest_to_canonical_catalog.py"
NEW_CLI = Path(__file__).resolve().parents[1] / "scripts" / "ingest_to_canonical_catalog.py"


def _run_legacy_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(  # noqa: S603
        [sys.executable, str(LEGACY_CLI), *args],
        capture_output=True,
        text=True,
        check=False,
    )


def test_legacy_emits_deprecation_warning_at_import() -> None:
    """Importing the legacy module emits DeprecationWarning."""
    import importlib.util

    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        spec = importlib.util.spec_from_file_location(
            "scripts.m061_ingest_to_canonical_catalog",
            str(LEGACY_CLI),
        )
        if spec is None or spec.loader is None:
            pytest.skip("Could not load legacy module spec")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)  # type: ignore[union-attr]
        deprecation_warnings = [
            w
            for w in w
            if issubclass(w.category, DeprecationWarning) and "m061" in str(w.message).lower()
        ]
        assert deprecation_warnings, "Expected DeprecationWarning mentioning m061"
        msg = str(deprecation_warnings[0].message)
        assert "deprecated" in msg.lower()
        assert "ingest_to_canonical_catalog" in msg


def test_legacy_help_shows_deprecation() -> None:
    result = _run_legacy_cli("--help")
    assert result.returncode == 0
    assert "[DEPRECATED]" in result.stdout


def test_legacy_forwards_no_index_flag() -> None:
    result = _run_legacy_cli("--no-network", "--no-index", "--report-path", "/tmp/legacy_nindex.md")
    assert result.returncode == 0, result.stderr
    assert "[DEPRECATED]" in result.stderr
    # Should produce identical output to new CLI
    assert "unique_arxiv_ids=32" in result.stdout
    assert "arxiv_api_requests=0" in result.stdout
    assert "skipped=32" in result.stdout


def test_legacy_no_args_matches_new_cli_no_args() -> None:
    """With no args, legacy CLI defaults match new CLI defaults."""
    legacy = _run_legacy_cli("--no-network", "--no-index")
    new = subprocess.run(  # noqa: S603
        [sys.executable, str(NEW_CLI), "--no-network", "--no-index"],
        capture_output=True,
        text=True,
        check=False,
    )
    assert legacy.returncode == new.returncode
    # Both should report same counts
    assert "unique_arxiv_ids=32" in legacy.stdout
    assert "unique_arxiv_ids=32" in new.stdout
    assert "skipped=32" in legacy.stdout
    assert "skipped=32" in new.stdout


def test_legacy_forwards_custom_paths(tmp_path: Path) -> None:
    custom_m061 = tmp_path / "m061"
    custom_m061.mkdir()
    report = tmp_path / "custom_report.md"
    result = _run_legacy_cli(
        "--no-network",
        "--no-index",
        "--m061-root",
        str(custom_m061),
        "--report-path",
        str(report),
    )
    assert result.returncode != 0
    assert "No M061" in result.stderr or "FileNotFoundError" in result.stderr


def test_legacy_trajectory_check_reference_preserved() -> None:
    """The trajectory check in scripts/check_project_trajectory.py still references legacy path."""
    trajectory = Path(__file__).resolve().parents[1] / "scripts" / "check_project_trajectory.py"
    content = trajectory.read_text()
    assert "m061_ingest_to_canonical_catalog" in content, (
        "Trajectory check must still reference legacy script path for audit trail"
    )
