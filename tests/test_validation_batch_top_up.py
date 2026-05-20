from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.validation_batch_state import (
    SelectedPaper,
    SourceReadiness,
    ValidationBatchState,
)
from arxiv_archive.validation_batch_workflow import (
    build_bounded_top_up_report,
    write_bounded_top_up_run,
)


def _state(ready_count: int, total: int = 3) -> ValidationBatchState:
    return ValidationBatchState(
        batch_id="top-up-batch",
        selected_papers=tuple(
            SelectedPaper(paper_id=f"p{i}", selection_role="deterministic_expansion", rank=i)
            for i in range(total)
        ),
        source_readiness_by_paper={
            f"p{i}": SourceReadiness(ready_for_markdown_scan=i < ready_count) for i in range(total)
        },
    )


def _inventory() -> dict:
    return {
        "candidates": [
            {"paper_id": "p0", "availability": {"ready_for_markdown_scan": True}},
            {
                "paper_id": "p3",
                "availability": {"ready_for_markdown_scan": False},
                "risk_tags": ["missing_markdown"],
            },
            {
                "paper_id": "p4",
                "availability": {"markdown_present": True, "markdown_quality_accepted": True},
            },
            {"paper_id": "p5", "availability": {"ready_for_markdown_scan": True}},
        ]
    }


def test_bounded_top_up_allows_already_full_quota() -> None:
    report = build_bounded_top_up_report(
        _state(ready_count=3),
        target_count=3,
        candidate_inventory=_inventory(),
        max_candidates_to_consider=2,
    )

    assert report["initial_accepted_ready_count"] == 3
    assert report["initial_shortage_count"] == 0
    assert report["accepted_replacement_count"] == 0
    assert report["remaining_shortage_count"] == 0
    assert report["scan_allowed"] is True
    assert report["raw_text_included"] is False


def test_bounded_top_up_selects_ready_replacements_deterministically() -> None:
    report = build_bounded_top_up_report(
        _state(ready_count=1),
        target_count=3,
        candidate_inventory=_inventory(),
        max_candidates_to_consider=3,
    )

    assert report["initial_accepted_ready_count"] == 1
    assert report["initial_shortage_count"] == 2
    assert [item["paper_id"] for item in report["accepted_replacements"]] == ["p4", "p5"]
    assert [item["paper_id"] for item in report["rejected_candidates"]] == ["p3"]
    assert report["considered_replacement_count"] == 3
    assert report["final_accepted_ready_count"] == 3
    assert report["remaining_shortage_count"] == 0
    assert report["scan_allowed"] is True


def test_bounded_top_up_blocks_when_max_candidates_exhausted() -> None:
    report = build_bounded_top_up_report(
        _state(ready_count=1),
        target_count=3,
        candidate_inventory=_inventory(),
        max_candidates_to_consider=1,
    )

    assert report["considered_replacement_count"] == 1
    assert report["accepted_replacement_count"] == 0
    assert report["rejected_replacement_count"] == 1
    assert report["remaining_shortage_count"] == 2
    assert report["scan_allowed"] is False
    assert report["blocker_count"] == 1


def test_bounded_top_up_excludes_selected_candidates() -> None:
    report = build_bounded_top_up_report(
        _state(ready_count=2),
        target_count=3,
        candidate_inventory=_inventory(),
        max_candidates_to_consider=4,
    )

    selected_ids = {"p0", "p1", "p2"}
    assert selected_ids.isdisjoint({item["paper_id"] for item in report["accepted_replacements"]})
    assert [item["paper_id"] for item in report["accepted_replacements"]] == ["p4"]
    assert report["scan_allowed"] is True


def test_write_bounded_top_up_run_writes_blocker_diagnostic(tmp_path: Path) -> None:
    report = build_bounded_top_up_report(
        _state(ready_count=1),
        target_count=3,
        candidate_inventory=_inventory(),
        max_candidates_to_consider=1,
    )

    paths = write_bounded_top_up_run(report, tmp_path, prefix="blocked")

    summary = json.loads(paths["summary_path"].read_text(encoding="utf-8"))
    diagnostics = paths["diagnostics_path"].read_text(encoding="utf-8").splitlines()
    assert summary["schema_version"] == "m009-bounded-top-up-summary.v1"
    assert summary["scan_allowed"] is False
    assert any("bounded_top_up_shortage" in line for line in diagnostics)
    assert "raw paper text" not in json.dumps(summary)
