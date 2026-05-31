from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.validation_batch_state import (
    SelectedPaper,
    SourceReadiness,
    ValidationBatchState,
)
from arxiv_archive.validation_batch_workflow import (
    build_source_preflight_summary,
    initialize_validation_batch,
    load_validation_manifest,
    preflight_validation_batch,
    selected_papers_from_manifest,
    source_readiness_for_paper,
    validation_batch_state_preview,
    write_source_preflight_run,
)


def _manifest(tmp_path: Path) -> Path:
    paper_dir = tmp_path / "paper"
    paper_dir.mkdir()
    full_text = paper_dir / "full_text.md"
    full_text.write_text("# Abstract\n\nRedacted fixture content for size only.\n", encoding="utf-8")
    manifest = {
        "papers": [
            {
                "paper_id": "2605.00001v1",
                "rank": 2,
                "selection_role": "deterministic_expansion",
                "risk_tags": ["missing_pdf"],
                "source_paths": {"research_full_text_md": str(full_text), "cache_pdf": None},
            },
            {
                "paper_id": "2605.00000v1",
                "rank": 1,
                "selection_role": "m005_baseline_overlap",
                "risk_tags": ["missing_markdown"],
                "source_paths": {"research_full_text_md": str(full_text)},
            },
        ]
    }
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    return path


def test_selected_papers_from_manifest_are_sorted_and_normalized(tmp_path: Path) -> None:
    manifest = load_validation_manifest(_manifest(tmp_path))

    papers = selected_papers_from_manifest(manifest)

    assert [paper.paper_id for paper in papers] == ["2605.00000v1", "2605.00001v1"]
    assert papers[0].selection_role == "baseline_overlap"
    assert papers[1].selection_role == "deterministic_expansion"
    assert papers[0].source_paths["research_full_text_md"].endswith("full_text.md")


def test_initialize_validation_batch_writes_state_and_selection_manifest(tmp_path: Path) -> None:
    result = initialize_validation_batch(
        manifest_path=_manifest(tmp_path),
        batch_id="b001",
        output_dir=tmp_path / "batches",
        limit=1,
    )

    state_payload = json.loads(result["state_path"].read_text(encoding="utf-8"))
    selection_payload = json.loads(result["selection_manifest_path"].read_text(encoding="utf-8"))
    assert state_payload["batch_id"] == "b001"
    assert state_payload["phase"] == "initialized"
    assert len(state_payload["selected_papers"]) == 1
    assert selection_payload["paper_count"] == 1
    assert selection_payload["production_import_attempted"] is False
    assert selection_payload["ladybugdb_written"] is False


def test_source_readiness_for_paper_detects_markdown_and_missing_pdf(tmp_path: Path) -> None:
    full_text = tmp_path / "full_text.md"
    full_text.write_text("content", encoding="utf-8")
    paper = SelectedPaper(
        paper_id="2605.00002v1",
        selection_role="deterministic_expansion",
        risk_tags=("missing_pdf",),
        source_paths={"research_full_text_md": str(full_text)},
    )

    readiness = source_readiness_for_paper(paper)

    assert readiness.markdown_present is True
    assert readiness.markdown_quality_accepted is True
    assert readiness.ready_for_markdown_scan is True
    assert readiness.pdf_present is False
    assert readiness.pdf_missing is True
    assert readiness.unavailable_source is False


def test_source_readiness_for_paper_uses_deterministic_fallback_paths(tmp_path: Path) -> None:
    fallback_root = tmp_path / "research"
    cache_root = tmp_path / "cache"
    paper_dir = fallback_root / "2605.00020v1"
    paper_dir.mkdir(parents=True)
    (paper_dir / "full_text.md").write_text("content", encoding="utf-8")
    cache_root.mkdir()
    (cache_root / "2605.00020v1.pdf").write_bytes(b"%PDF-1.4")
    paper = SelectedPaper(paper_id="2605.00020v1", selection_role="deterministic_expansion")

    readiness = source_readiness_for_paper(paper, fallback_root=fallback_root, cache_root=cache_root)

    assert readiness.markdown_present is True
    assert readiness.markdown_quality_accepted is True
    assert readiness.pdf_present is True
    assert readiness.pdf_missing is False
    assert readiness.ready_for_markdown_scan is True



def test_source_readiness_for_paper_carries_loader_provenance(tmp_path: Path) -> None:
    full_text = tmp_path / "full_text.md"
    full_text.write_text("# Abstract\n\nLoader provenance is explicit.\n", encoding="utf-8")
    pdf = tmp_path / "paper.pdf"
    pdf.write_bytes(b"%PDF-1.4 fixture")
    paper = SelectedPaper(
        paper_id="2605.00030v1",
        selection_role="deterministic_expansion",
        source_paths={"research_full_text_md": str(full_text), "cache_pdf": str(pdf)},
    )

    readiness = source_readiness_for_paper(paper)

    assert readiness.ready_for_markdown_scan is True
    assert readiness.pdf_present is True
    markdown = readiness.loader_provenance_by_role["markdown"]
    pdf_provenance = readiness.loader_provenance_by_role["pdf"]
    assert markdown["source_type"] == "markdown"
    assert markdown["source_path"] == str(full_text)
    assert markdown["sha256"]
    assert markdown["outcome"] == "loaded"
    assert markdown["failure_reason"] is None
    assert pdf_provenance["source_type"] == "pdf"
    assert pdf_provenance["media_type"] == "application/pdf"
    assert pdf_provenance["outcome"] == "loaded_metadata_only"


def test_source_readiness_for_paper_records_missing_source_failure(tmp_path: Path) -> None:
    missing_markdown = tmp_path / "missing.md"
    missing_pdf = tmp_path / "missing.pdf"
    paper = SelectedPaper(
        paper_id="2605.00031v1",
        selection_role="deterministic_expansion",
        source_paths={"research_full_text_md": str(missing_markdown), "cache_pdf": str(missing_pdf)},
    )

    readiness = source_readiness_for_paper(paper)

    assert readiness.ready_for_markdown_scan is False
    assert readiness.unavailable_source is True
    assert readiness.loader_provenance_by_role["markdown"]["failure_reason"] == "source_missing"
    assert readiness.loader_provenance_by_role["markdown"]["selected_fallback"] == "source_missing"
    assert readiness.loader_provenance_by_role["pdf"]["failure_reason"] == "source_missing"

def test_preflight_validation_batch_adds_contradiction_diagnostics(tmp_path: Path) -> None:
    full_text = tmp_path / "full_text.md"
    full_text.write_text("content", encoding="utf-8")
    state = ValidationBatchState(
        batch_id="b002",
        phase="initialized",
        selected_papers=(
            SelectedPaper(
                paper_id="2605.00003v1",
                selection_role="deterministic_expansion",
                risk_tags=("missing_markdown",),
                source_paths={"research_full_text_md": str(full_text)},
            ),
        ),
    )

    preflighted = preflight_validation_batch(state)

    assert preflighted.phase == "source_ready"
    assert preflighted.source_readiness_by_paper["2605.00003v1"].ready_for_markdown_scan is True
    assert [diagnostic["code"] for diagnostic in preflighted.diagnostics] == [
        "ready_with_missing_markdown_risk_tag"
    ]


def test_write_source_preflight_run_writes_redacted_summary_and_diagnostics(tmp_path: Path) -> None:
    state = ValidationBatchState(
        batch_id="b003",
        phase="source_ready",
        selected_papers=(SelectedPaper(paper_id="2605.00004v1", selection_role="baseline_overlap"),),
        source_readiness_by_paper={
            "2605.00004v1": SourceReadiness(
                markdown_present=True,
                markdown_quality_accepted=True,
                pdf_missing=True,
                ready_for_markdown_scan=True,
            )
        },
    )

    paths = write_source_preflight_run(state, tmp_path / "out")

    summary = json.loads(paths["summary_path"].read_text(encoding="utf-8"))
    diagnostics = paths["diagnostics_path"].read_text(encoding="utf-8")
    assert summary["paper_count"] == 1
    assert summary["ready_for_markdown_scan_count"] == 1
    assert summary["pdf_missing_count"] == 1
    assert summary["production_import_attempted"] is False
    assert diagnostics == ""


def test_build_source_preflight_summary_counts_blockers() -> None:
    state = ValidationBatchState(
        batch_id="b004",
        phase="source_blocked",
        diagnostics=(
            {"severity": "blocker", "code": "ready_without_markdown", "message": "x", "recommended_action": "y"},
            {"severity": "warning", "code": "conflicting_pdf_state", "message": "x", "recommended_action": "y"},
        ),
    )

    summary = build_source_preflight_summary(state)

    assert summary["blocker_count"] == 1
    assert summary["warning_count"] == 1
    assert summary["raw_text_included"] is False


def test_validation_batch_state_preview_is_compact_and_redacted() -> None:
    state = ValidationBatchState(
        batch_id="b005",
        selected_papers=(SelectedPaper(paper_id="2605.00005v1", selection_role="retry"),),
    )

    preview = validation_batch_state_preview(state)

    assert preview == {
        "schema_version": "m007-validation-batch-state.v1",
        "batch_id": "b005",
        "phase": "planned",
        "paper_count": 1,
        "diagnostic_count": 0,
        "raw_text_included": False,
        "chunk_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "optimizer_traces_included": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }
