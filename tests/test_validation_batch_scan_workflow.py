from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.validation_batch_state import (
    SelectedPaper,
    SourceReadiness,
    ValidationBatchState,
)
from arxiv_archive.validation_batch_workflow import (
    _mixed_benchmark_context,
    build_delta_report,
    build_outlier_report,
    run_validation_batch_scan,
    scan_import_gate_diagnostics,
    write_validation_scan_manifest,
    write_validation_scan_source_readiness,
)


def _state(tmp_path: Path) -> ValidationBatchState:
    paper_dir = tmp_path / "papers" / "2605.00001v1"
    paper_dir.mkdir(parents=True)
    (paper_dir / "full_text.md").write_text(
        "# Abstract\n\nThis paper proposes a method.\n\n# Results\n\nThe method improves accuracy.\n",
        encoding="utf-8",
    )
    return ValidationBatchState(
        batch_id="fixture-b001",
        phase="source_ready",
        selected_papers=(
            SelectedPaper(
                paper_id="2605.00001v1",
                selection_role="deterministic_expansion",
                rank=1,
                risk_tags=("missing_pdf",),
                source_paths={
                    "research_workspace": str(paper_dir),
                    "research_full_text_md": str(paper_dir / "full_text.md"),
                },
            ),
        ),
        source_readiness_by_paper={
            "2605.00001v1": SourceReadiness(
                markdown_present=True,
                markdown_quality_accepted=True,
                pdf_missing=True,
                ready_for_markdown_scan=True,
            )
        },
    )


def test_write_validation_scan_manifest_is_redacted(tmp_path: Path) -> None:
    path = write_validation_scan_manifest(_state(tmp_path), tmp_path / "manifest.json")

    payload = json.loads(path.read_text(encoding="utf-8"))

    assert payload["schema_version"] == "m007-validation-scan-manifest.v1"
    assert payload["paper_count"] == 1
    assert payload["raw_text_included"] is False
    assert payload["production_import_attempted"] is False
    assert "This paper proposes" not in json.dumps(payload)


def test_write_validation_scan_source_readiness_adapts_preflight_summary(tmp_path: Path) -> None:
    path = write_validation_scan_source_readiness(_state(tmp_path), tmp_path / "source.json")

    payload = json.loads(path.read_text(encoding="utf-8"))

    assert payload["paper_count"] == 1
    assert payload["ready_for_markdown_scan_count"] == 1
    assert payload["still_missing_markdown_count"] == 0
    assert payload["available_pdf_count"] == 0
    assert payload["ladybugdb_written"] is False


def test_run_validation_batch_scan_writes_redacted_artifacts(tmp_path: Path) -> None:
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

    result = run_validation_batch_scan(
        _state(tmp_path),
        tmp_path / "out",
        structure_baseline_path=baseline_path,
        milestone_id="M009-fh0tg0",
    )

    summary = json.loads(result["summary_path"].read_text(encoding="utf-8"))
    diagnostics = result["diagnostics_path"].read_text(encoding="utf-8")
    delta = json.loads(result["delta_report_path"].read_text(encoding="utf-8"))
    outliers = json.loads(result["outlier_report_path"].read_text(encoding="utf-8"))
    state = result["state"]
    assert state.phase == "scanned"
    assert state.artifact_paths.aggregate_summary_json == str(result["summary_path"])
    assert summary["schema_version"] == "m007-validation-scan-summary.v1"
    assert summary["milestone"] == "M009-fh0tg0"
    assert summary["milestone_id"] == "M009-fh0tg0"
    assert summary["batch_id"] == "fixture-b001"
    assert summary["paper_count"] == 1
    assert summary["aggregate"]["chunk_count"] > 0
    assert summary["aggregate"]["import_eligible_chunk_count"] == 0
    assert delta["milestone_id"] == "M009-fh0tg0"
    assert delta["batch_id"] == "fixture-b001"
    assert delta["structure_aware_baseline"]["available"] is True
    assert outliers["milestone_id"] == "M009-fh0tg0"
    assert outliers["batch_id"] == "fixture-b001"
    assert outliers["schema_version"] == "m007-validation-outlier-report.v1"
    assert diagnostics
    assert "This paper proposes" not in diagnostics
    assert summary["raw_text_included"] is False
    assert summary["ladybugdb_written"] is False


def test_run_validation_batch_scan_rejects_unready_state(tmp_path: Path) -> None:
    state = ValidationBatchState(batch_id="b002", phase="source_blocked")

    try:
        run_validation_batch_scan(state, tmp_path / "out")
    except ValueError as exc:
        assert "source_ready" in str(exc)
    else:
        raise AssertionError("expected ValueError for unready scan state")


def test_scan_import_gate_diagnostics_blocks_unexpected_import_eligibility() -> None:
    diagnostics = scan_import_gate_diagnostics({"aggregate": {"import_eligible_chunk_count": 3}})

    assert diagnostics == [
        {
            "severity": "blocker",
            "code": "unexpected_import_eligible_chunks",
            "message": "Validation scan produced 3 import-eligible chunks outside a reviewed promotion path.",
            "recommended_action": "Stop automation and run independent review before any KG import work.",
        }
    ]


def test_build_delta_and_outlier_reports_are_safe() -> None:
    scan = {
        "paper_count": 1,
        "aggregate": {
            "chunk_count": 10,
            "import_eligible_chunk_count": 0,
            "counts_by_route": {"retrieval_only": 8, "claim_extraction": 2},
            "refusal_counts": {"retrieval_only_not_import_ready": 8},
        },
        "records": [{"paper_id": "p1", "chunks_per_10k_bytes": 4.2}],
        "outliers": [{"paper_id": "p1", "flags": ["claim_candidate_heavy"], "chunk_count": 10}],
    }

    delta = build_delta_report(scan)
    outliers = build_outlier_report(scan)

    mixed = _mixed_benchmark_context(
        {"chunk_count": 10, "import_eligible_chunk_count": 0},
        {"aggregate": {"total_chunk_count": 7, "total_import_eligible_chunk_count": 0}},
    )

    assert delta["raw_text_included"] is False
    assert delta["structure_aware_baseline"]["available"] is False
    assert mixed["benchmark_chunk_count"] == 7
    assert mixed["chunk_count_delta"] == 3
    assert outliers["outliers"][0]["chunks_per_10k_bytes"] == 4.2
    assert outliers["production_import_attempted"] is False
