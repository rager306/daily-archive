from __future__ import annotations

import json
import subprocess
from pathlib import Path


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["uv", "run", "python", "-m", "arxiv_archive", *args],
        check=False,
        capture_output=True,
        text=True,
    )


def _manifest(tmp_path: Path) -> Path:
    paper_dir = tmp_path / "paper"
    paper_dir.mkdir()
    full_text = paper_dir / "full_text.md"
    full_text.write_text("# Abstract\n\nFixture markdown.\n", encoding="utf-8")
    manifest = {
        "papers": [
            {
                "paper_id": "2605.00001v1",
                "rank": 1,
                "selection_role": "deterministic_expansion",
                "risk_tags": ["missing_pdf"],
                "source_paths": {"research_full_text_md": str(full_text)},
            }
        ]
    }
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    return path


def test_validation_batch_init_writes_state_and_selection_manifest(tmp_path: Path) -> None:
    output_dir = tmp_path / "batches"

    result = _run_cli(
        "validation-batch",
        "init",
        "--batch-id",
        "fixture-b001",
        "--manifest-path",
        str(_manifest(tmp_path)),
        "--output-dir",
        str(output_dir),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    state_path = Path(payload["state_path"])
    selection_manifest_path = Path(payload["selection_manifest_path"])
    assert payload["status"] == "initialized"
    assert payload["paper_count"] == 1
    assert payload["real_source_acquisition_performed"] is False
    assert payload["real_scan_performed"] is False
    assert state_path.exists()
    assert selection_manifest_path.exists()


def test_validation_batch_preflight_updates_state_and_writes_summary(tmp_path: Path) -> None:
    output_dir = tmp_path / "batches"
    init_result = _run_cli(
        "validation-batch",
        "init",
        "--batch-id",
        "fixture-b002",
        "--manifest-path",
        str(_manifest(tmp_path)),
        "--output-dir",
        str(output_dir),
        "--json",
    )
    state_path = Path(json.loads(init_result.stdout)["state_path"])

    result = _run_cli(
        "validation-batch",
        "preflight",
        "--state-path",
        str(state_path),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    summary_path = Path(payload["summary_path"])
    diagnostics_path = Path(payload["diagnostics_path"])
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert payload["status"] == "preflighted"
    assert payload["phase"] == "source_ready"
    assert summary["paper_count"] == 1
    assert summary["ready_for_markdown_scan_count"] == 1
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert diagnostics_path.exists()


def test_validation_batch_preflight_can_write_to_explicit_output_dir(tmp_path: Path) -> None:
    init_result = _run_cli(
        "validation-batch",
        "init",
        "--batch-id",
        "fixture-b003",
        "--manifest-path",
        str(_manifest(tmp_path)),
        "--output-dir",
        str(tmp_path / "init"),
        "--json",
    )
    state_path = Path(json.loads(init_result.stdout)["state_path"])
    preflight_dir = tmp_path / "preflight"

    result = _run_cli(
        "validation-batch",
        "preflight",
        "--state-path",
        str(state_path),
        "--output-dir",
        str(preflight_dir),
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert Path(payload["state_path"]).parent == preflight_dir
    assert Path(payload["summary_path"]).parent == preflight_dir
