from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.validation_batch_state import (
    SelectedPaper,
    SourceReadiness,
    ValidationBatchState,
)
from arxiv_archive.validation_batch_workflow import build_quota_fill_report, write_quota_fill_run


def test_build_quota_fill_report_allows_full_ready_quota() -> None:
    state = ValidationBatchState(
        batch_id="quota-ok",
        selected_papers=tuple(
            SelectedPaper(paper_id=f"p{i}", selection_role="deterministic_expansion", rank=i)
            for i in range(10)
        ),
        source_readiness_by_paper={
            f"p{i}": SourceReadiness(ready_for_markdown_scan=True) for i in range(10)
        },
    )

    report = build_quota_fill_report(state, target_count=10)

    assert report["target_count"] == 10
    assert report["attempted_count"] == 10
    assert report["accepted_ready_count"] == 10
    assert report["rejected_count"] == 0
    assert report["shortage_count"] == 0
    assert report["scan_allowed"] is True
    assert report["raw_text_included"] is False
    assert {record["outcome"] for record in report["records"]} == {"accepted_ready"}


def test_build_quota_fill_report_blocks_shortage_and_lists_replacements() -> None:
    state = ValidationBatchState(
        batch_id="quota-short",
        selected_papers=tuple(
            SelectedPaper(paper_id=f"p{i}", selection_role="deterministic_expansion", rank=i)
            for i in range(3)
        ),
        source_readiness_by_paper={
            "p0": SourceReadiness(ready_for_markdown_scan=True),
            "p1": SourceReadiness(ready_for_markdown_scan=False),
        },
    )
    inventory = {
        "candidates": [
            {"paper_id": "p0", "availability": {"markdown_present": True}},
            {"paper_id": "p3", "availability": {"markdown_present": True}},
            {"paper_id": "p4", "availability": {"markdown_present": False}},
        ]
    }

    report = build_quota_fill_report(state, target_count=3, candidate_inventory=inventory)

    assert report["accepted_ready_count"] == 1
    assert report["rejected_count"] == 2
    assert report["shortage_count"] == 2
    assert report["scan_allowed"] is False
    assert [candidate["paper_id"] for candidate in report["replacement_candidates"]] == ["p3", "p4"]
    assert {record["outcome"] for record in report["records"]} == {
        "accepted_ready",
        "rejected_not_source_ready",
        "rejected_no_preflight",
    }


def test_write_quota_fill_run_writes_redacted_summary_and_diagnostics(tmp_path: Path) -> None:
    state = ValidationBatchState(
        batch_id="quota-short",
        selected_papers=(
            SelectedPaper(paper_id="p0", selection_role="deterministic_expansion", rank=0),
        ),
        source_readiness_by_paper={"p0": SourceReadiness(ready_for_markdown_scan=False)},
    )
    report = build_quota_fill_report(state, target_count=1)

    paths = write_quota_fill_run(report, tmp_path)

    summary = json.loads(paths["summary_path"].read_text())
    diagnostics = paths["diagnostics_path"].read_text().splitlines()
    assert summary["target_count"] == 1
    assert summary["accepted_ready_count"] == 0
    assert summary["shortage_count"] == 1
    assert summary["raw_text_included"] is False
    assert "records" not in summary
    assert any("quota_shortage" in line for line in diagnostics)
    assert any("rejected_not_source_ready" in line for line in diagnostics)
