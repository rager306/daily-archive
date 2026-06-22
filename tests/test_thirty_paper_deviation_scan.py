from __future__ import annotations

import json
from pathlib import Path

from research_graph.infrastructure.corpus.sources.thirty_paper_deviation_scan import (
    build_thirty_paper_deviation_scan,
    write_thirty_paper_deviation_run,
)


def _write_manifest(tmp_path: Path) -> Path:
    paper_a = tmp_path / "papers" / "2605.00001v1"
    paper_b = tmp_path / "papers" / "2605.00002v1"
    paper_a.mkdir(parents=True)
    paper_b.mkdir(parents=True)
    (paper_a / "full_text.md").write_text(
        "# Introduction\n\nThis paper proposes a method.\n\n# Results\n\nThe method improves accuracy.\n",
        encoding="utf-8",
    )
    (paper_b / "full_text.md").write_text(
        "# Abstract\n\nFigure 1: Overview of the system.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n[1] A reference entry.\n",
        encoding="utf-8",
    )
    manifest = {
        "paper_count": 2,
        "m005_overlap_count": 1,
        "expansion_count": 1,
        "papers": [
            {
                "rank": 1,
                "paper_id": "2605.00001v1",
                "title": "A",
                "selection_role": "m005_baseline_overlap",
                "risk_tags": ["baseline"],
                "source_paths": {"research_workspace": str(paper_a)},
            },
            {
                "rank": 2,
                "paper_id": "2605.00002v1",
                "title": "B",
                "selection_role": "deterministic_expansion",
                "risk_tags": ["table_figure"],
                "source_paths": {"research_workspace": str(paper_b)},
            },
        ],
    }
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    return path


def _write_source_summary(tmp_path: Path) -> Path:
    summary = {
        "paper_count": 2,
        "ready_for_markdown_scan_count": 2,
        "still_missing_markdown_count": 0,
        "available_pdf_count": 1,
    }
    path = tmp_path / "source-summary.json"
    path.write_text(json.dumps(summary), encoding="utf-8")
    return path


def test_build_thirty_paper_deviation_scan_emits_redacted_metrics(tmp_path: Path) -> None:
    scan = build_thirty_paper_deviation_scan(
        manifest_path=_write_manifest(tmp_path),
        source_acquisition_summary_path=_write_source_summary(tmp_path),
    )

    assert scan["paper_count"] == 2
    assert scan["source_readiness"]["ready_for_markdown_scan_count"] == 2
    assert scan["aggregate"]["paper_count"] == 2
    assert scan["aggregate"]["chunk_count"] > 0
    assert scan["aggregate"]["markdown_byte_size_total"] > 0
    assert scan["aggregate"]["import_eligible_chunk_count"] == 0
    assert scan["raw_text_included"] is False
    assert scan["production_import_attempted"] is False
    assert len(scan["records"]) == 2
    serialized = json.dumps(scan)
    assert "This paper proposes a method" not in serialized
    assert "The method improves accuracy" not in serialized
    assert "Figure 1: Overview" not in serialized


def test_build_thirty_paper_deviation_scan_compares_baseline(tmp_path: Path) -> None:
    baseline = {
        "aggregate": {
            "total_chunk_count": 3,
            "total_import_eligible_chunk_count": 0,
            "paper_count": 1,
        }
    }
    baseline_path = tmp_path / "baseline.json"
    baseline_path.write_text(json.dumps(baseline), encoding="utf-8")

    scan = build_thirty_paper_deviation_scan(
        manifest_path=_write_manifest(tmp_path),
        baseline_summary_path=baseline_path,
    )

    comparison = scan["baseline_comparison"]
    assert comparison["baseline_available"] is True
    assert comparison["baseline_chunk_count"] == 3
    assert comparison["current_paper_count"] == 2
    assert comparison["current_import_eligible_chunk_count"] == 0


def test_write_thirty_paper_deviation_run_splits_summary_and_diagnostics(tmp_path: Path) -> None:
    scan = build_thirty_paper_deviation_scan(manifest_path=_write_manifest(tmp_path))

    paths = write_thirty_paper_deviation_run(scan, tmp_path / "out")

    summary = json.loads(paths["summary_path"].read_text(encoding="utf-8"))
    diagnostics = [json.loads(line) for line in paths["diagnostics_path"].read_text(encoding="utf-8").splitlines()]
    assert paths["summary_path"].name == "thirty-paper-deviation-summary.json"
    assert paths["diagnostics_path"].name == "thirty-paper-deviation-diagnostics.jsonl"
    assert "records" not in summary
    assert summary["paper_count"] == 2
    assert len(diagnostics) == 2
    assert all(record["raw_text_included"] is False for record in diagnostics)
