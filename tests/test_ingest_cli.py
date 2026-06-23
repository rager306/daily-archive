"""Tests for scripts/ingest_to_canonical_catalog.py CLI.

Validates CLI flag handling + offline (no-network) end-to-end behavior.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

CLI_PATH = Path(__file__).resolve().parents[1] / "scripts" / "ingest_to_canonical_catalog.py"


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    """Run the CLI as a subprocess with the given args."""
    return subprocess.run(  # noqa: S603
        [sys.executable, str(CLI_PATH), *args],
        capture_output=True,
        text=True,
        check=False,
    )


def test_cli_help() -> None:
    result = _run_cli("--help")
    assert result.returncode == 0
    assert "Ingest M061-acquired PDFs" in result.stdout
    assert "--no-network" in result.stdout
    assert "--m061-root" in result.stdout


def test_cli_no_network_offline(tmp_path: Path) -> None:
    """CLI with --no-network --no-index must run end-to-end with no network calls."""
    report = tmp_path / "report.md"
    result = _run_cli(
        "--no-network",
        "--no-index",
        "--report-path",
        str(report),
    )
    assert result.returncode == 0, result.stderr
    assert "arxiv_api_requests=0" in result.stdout
    assert "arxiv_api_429s=0" in result.stdout
    # With --no-network, all metadata is fallback
    assert "fallback=" in result.stdout
    assert report.exists()
    content = report.read_text()
    assert "M061 S04 canonical catalog ingestion report" in content
    # Fail-closed invariant preserved: no graph writes
    assert "Graph writes is not authorized" in content


def test_cli_no_index_skips_index_update(tmp_path: Path) -> None:
    """--no-index flag must work without errors."""
    report = tmp_path / "report.md"
    result = _run_cli(
        "--no-network",
        "--no-index",
        "--report-path",
        str(report),
    )
    assert result.returncode == 0
    # index_updated status is rendered in report
    content = report.read_text()
    assert "index.json updated:" in content


def test_cli_default_invokes_real_m061_artifact(tmp_path: Path) -> None:
    """Default invocation reads from artifacts/m061-2hop/ (already-cataloged state)."""
    report = tmp_path / "report.md"
    # NOTE: This test runs against the project's real M061 artifact tree.
    # Since catalog already has all 32 PDFs ingested, all records will be 'skipped'.
    result = _run_cli(
        "--no-network",
        "--no-index",
        "--report-path",
        str(report),
    )
    assert result.returncode == 0
    assert "unique_arxiv_ids=32" in result.stdout
    assert "skipped=32" in result.stdout


def test_cli_custom_m061_root_nonexistent(tmp_path: Path) -> None:
    """Custom --m061-root pointing at empty dir must fail with FileNotFoundError."""
    empty = tmp_path / "empty"
    empty.mkdir()
    result = _run_cli(
        "--m061-root",
        str(empty),
        "--no-network",
        "--no-index",
    )
    assert result.returncode != 0
    assert "FileNotFoundError" in result.stderr or "No M061" in result.stderr
