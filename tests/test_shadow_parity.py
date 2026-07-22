"""M200 S02: shadow parity contract tests (no M001 artifact writes)."""

from __future__ import annotations

from datetime import date
from pathlib import Path

from research_graph.application.analyze_source import AnalyzeSourceResult
from research_graph.application.shadow_parity import (
    LegacyMetadataSnapshot,
    SourceTextRecord,
    compare_day_shadow,
    run_shadow_parity,
)

_RUN = date(2026, 5, 14)
_TEXT = ("transformers attention reasoning", "attention mechanisms improve quality")


def test_compare_day_shadow_match() -> None:
    legacy = LegacyMetadataSnapshot(
        run_date=_RUN,
        paper_ids=("a", "b"),
        status="done",
        papers_fetched=2,
    )
    results = (
        AnalyzeSourceResult(source_id="a", status="done", stage_names=("statistical_pre_processor",)),
        AnalyzeSourceResult(source_id="b", status="done", stage_names=("statistical_pre_processor",)),
    )
    report = compare_day_shadow(legacy=legacy, canonical_results=results)
    assert report.match is True
    assert report.status == "match"
    assert report.differences == ()
    assert report.safety["m001_artifacts_mutated"] is False


def test_compare_day_shadow_empty_day() -> None:
    legacy = LegacyMetadataSnapshot(
        run_date=_RUN, paper_ids=(), status="empty", papers_fetched=0
    )
    report = compare_day_shadow(legacy=legacy, canonical_results=())
    assert report.match is True


def test_compare_day_shadow_id_mismatch() -> None:
    legacy = LegacyMetadataSnapshot(
        run_date=_RUN, paper_ids=("a",), status="done", papers_fetched=1
    )
    results = (AnalyzeSourceResult(source_id="b", status="done"),)
    report = compare_day_shadow(legacy=legacy, canonical_results=results)
    assert report.match is False
    assert any("missing_in_canonical:a" in d for d in report.differences)
    assert any("extra_in_canonical:b" in d for d in report.differences)


def test_compare_day_shadow_count_mismatch() -> None:
    legacy = LegacyMetadataSnapshot(
        run_date=_RUN, paper_ids=("a", "b"), status="done", papers_fetched=2
    )
    results = (AnalyzeSourceResult(source_id="a", status="done"),)
    report = compare_day_shadow(legacy=legacy, canonical_results=results)
    assert report.match is False
    assert any("count_mismatch" in d or "missing" in d for d in report.differences)


def test_run_shadow_parity_executes_analyze_source() -> None:
    legacy = LegacyMetadataSnapshot(
        run_date=_RUN, paper_ids=("src1",), status="done", papers_fetched=1
    )
    sources = [SourceTextRecord(source_id="src1", text_parts=_TEXT)]
    report = run_shadow_parity(legacy=legacy, sources=sources)
    assert report.match is True
    assert report.canonical_done_count == 1
    assert report.canonical_results[0].status == "done"
    assert report.safety["m001_artifacts_mutated"] is False
    assert report.safety["graph_writes_authorized"] is False


def test_run_shadow_parity_does_not_write_m001_dirs(tmp_path: Path, monkeypatch) -> None:
    queue = tmp_path / "queue"
    sessions = tmp_path / "sessions"
    queue.mkdir()
    sessions.mkdir()
    monkeypatch.setenv("HOME", str(tmp_path))
    # Even if callers pointed paths at tmp, shadow_parity must not create files.
    before_q = set(queue.iterdir())
    before_s = set(sessions.iterdir())
    legacy = LegacyMetadataSnapshot(
        run_date=_RUN, paper_ids=("x",), status="done", papers_fetched=1
    )
    run_shadow_parity(
        legacy=legacy,
        sources=[SourceTextRecord(source_id="x", text_parts=_TEXT)],
    )
    assert set(queue.iterdir()) == before_q
    assert set(sessions.iterdir()) == before_s
