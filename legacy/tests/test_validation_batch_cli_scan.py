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
    full_text.write_text(
        "# Abstract\n\nThis paper proposes a method.\n\n# Results\n\nThe method improves accuracy.\n",
        encoding="utf-8",
    )
    manifest = {
        "papers": [
            {
                "paper_id": "2605.00001v1",
                "rank": 1,
                "selection_role": "deterministic_expansion",
                "risk_tags": ["missing_pdf"],
                "source_paths": {
                    "research_workspace": str(paper_dir),
                    "research_full_text_md": str(full_text),
                },
            }
        ]
    }
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    return path


def _source_ready_state(tmp_path: Path) -> Path:
    init_result = _run_cli(
        "validation-batch",
        "init",
        "--batch-id",
        "fixture-bscan",
        "--manifest-path",
        str(_manifest(tmp_path)),
        "--output-dir",
        str(tmp_path / "init"),
        "--json",
    )
    state_path = Path(json.loads(init_result.stdout)["state_path"])
    preflight_result = _run_cli(
        "validation-batch",
        "preflight",
        "--state-path",
        str(state_path),
        "--output-dir",
        str(tmp_path / "preflight"),
        "--json",
    )
    assert preflight_result.returncode == 0, preflight_result.stderr
    return Path(json.loads(preflight_result.stdout)["state_path"])


def test_validation_batch_scan_cli_writes_scan_delta_and_outlier_artifacts(tmp_path: Path) -> None:
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(
        json.dumps(
            {
                "paper_count": 1,
                "chunk_count": 2,
                "import_eligible_chunk_count": 0,
                "counts_by_route": {"retrieval_only": 2},
                "refusal_counts": {"retrieval_only_not_import_ready": 2},
            }
        ),
        encoding="utf-8",
    )
    state_path = _source_ready_state(tmp_path)

    result = _run_cli(
        "validation-batch",
        "scan",
        "--state-path",
        str(state_path),
        "--output-dir",
        str(tmp_path / "scan"),
        "--structure-baseline-path",
        str(baseline_path),
        "--milestone-id",
        "M009-fh0tg0",
        "--json",
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["status"] == "scanned"
    assert payload["real_source_acquisition_performed"] is False
    assert payload["real_scan_performed"] is True
    assert payload["production_import_attempted"] is False
    assert payload["ladybugdb_written"] is False
    summary_path = Path(payload["summary_path"])
    delta_path = Path(payload["delta_report_path"])
    outlier_path = Path(payload["outlier_report_path"])
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    delta = json.loads(delta_path.read_text(encoding="utf-8"))
    outliers = json.loads(outlier_path.read_text(encoding="utf-8"))
    assert summary["schema_version"] == "m007-validation-scan-summary.v1"
    assert summary["milestone"] == "M009-fh0tg0"
    assert summary["milestone_id"] == "M009-fh0tg0"
    assert summary["batch_id"] == "fixture-bscan"
    assert summary["paper_count"] == 1
    assert summary["aggregate"]["import_eligible_chunk_count"] == 0
    assert delta["milestone_id"] == "M009-fh0tg0"
    assert delta["batch_id"] == "fixture-bscan"
    assert delta["structure_aware_baseline"]["available"] is True
    assert outliers["milestone_id"] == "M009-fh0tg0"
    assert outliers["batch_id"] == "fixture-bscan"
    assert outliers["schema_version"] == "m007-validation-outlier-report.v1"


def test_validation_batch_scan_rejects_source_blocked_state(tmp_path: Path) -> None:
    blocked_state = tmp_path / "blocked-state.json"
    blocked_state.write_text(
        json.dumps(
            {
                "schema_version": "m007-validation-batch-state.v1",
                "batch_id": "blocked",
                "phase": "source_blocked",
                "selected_papers": [],
                "input_manifests": [],
                "artifact_paths": {},
                "source_readiness_by_paper": {},
                "review": {},
                "recommendation": {},
                "safety": {},
                "diagnostics": [],
            }
        ),
        encoding="utf-8",
    )

    result = _run_cli(
        "validation-batch",
        "scan",
        "--state-path",
        str(blocked_state),
        "--output-dir",
        str(tmp_path / "scan"),
        "--json",
    )

    assert result.returncode != 0
    assert "source_ready" in result.stderr
