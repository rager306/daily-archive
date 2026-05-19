from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.graph_readiness_export import build_package_from_manifest_document
from arxiv_archive.graph_readiness_review import (
    generate_review_bundles,
    render_review_bundle,
    select_review_papers,
    validate_review_artifacts,
)

FIXTURE_TEXT = """# Test Paper

## Abstract

This paper introduces a bounded review fixture.

## Results

The fixture preserves a measurable result with source spans.

## References

[1] Example Reference.
"""

TABLE_TEXT = """# Table Paper

## Results

The paper includes a result.

## Supplementary Table S1

| Metric | Value |
|---|---:|
| Accuracy | 0.91 |
"""

SPLIT_REVIEW_TEXT = """# Split Review Paper

## Results

{}.

{}.

{}.
""".format(
    " ".join(f"alpha{i}" for i in range(70)),
    " ".join(f"beta{i}" for i in range(80)),
    " ".join(f"gamma{i}" for i in range(75)),
)


def _paper_dir(tmp_path: Path, paper_id: str, text: str) -> Path:
    paper_dir = tmp_path / "papers" / paper_id
    paper_dir.mkdir(parents=True)
    (paper_dir / "full_text.md").write_text(text, encoding="utf-8")
    (paper_dir / "full_text.method").write_text("docling", encoding="utf-8")
    return paper_dir


def _manifest_doc(paper_dir: Path, paper_id: str, title: str | None = None) -> dict[str, object]:
    return {
        "rank": 1,
        "paper_id": paper_id,
        "title": title or f"Fixture {paper_id}",
        "paper_dir": str(paper_dir),
        "expected_full_text_path": str(paper_dir / "full_text.md"),
    }


def _write_corpus(tmp_path: Path, docs: list[dict[str, object]]) -> Path:
    corpus_path = tmp_path / "corpus.json"
    corpus_path.write_text(json.dumps({"documents": docs}), encoding="utf-8")
    return corpus_path


def test_select_review_papers_includes_required_baseline_and_complex_candidates(tmp_path: Path) -> None:
    docs = []
    for paper_id in ["2605.14259v1", "2605.14517v1", "2605.14995v1"]:
        paper_dir = _paper_dir(tmp_path, paper_id, FIXTURE_TEXT)
        docs.append(_manifest_doc(paper_dir, paper_id))
    table_dir = _paper_dir(tmp_path, "2605.tablev1", TABLE_TEXT)
    docs.append(_manifest_doc(table_dir, "2605.tablev1"))
    packages = [build_package_from_manifest_document(doc, run_id="test-run") for doc in docs]

    selected = select_review_papers(packages)

    assert "2605.14259v1" in selected
    assert "2605.14517v1" in selected
    assert "2605.14995v1" in selected
    assert "2605.tablev1" in selected


def test_render_review_bundle_contains_bounded_snippets_and_route_metadata(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.14517v1", FIXTURE_TEXT)
    doc = _manifest_doc(paper_dir, "2605.14517v1", title="Dimension-Level Fixture")
    package = build_package_from_manifest_document(doc, run_id="test-run")

    rendered = render_review_bundle(package, doc, snippet_chars=80)

    assert "Independent Review Bundle — 2605.14517v1" in rendered
    assert "Dimension-Level Fixture" in rendered
    assert "Route Summary" in rendered
    assert "Chunk Samples" in rendered
    assert "source_span" in rendered
    assert "bounded review fixture" in rendered
    assert "Reviewer Output Contract" in rendered
    assert "do not leave placeholders" in rendered
    assert "manifest_implications" in rendered
    assert "Reviewer Verdict Placeholder" not in rendered


def test_render_review_bundle_prioritizes_split_candidates(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.splitv1", SPLIT_REVIEW_TEXT)
    doc = _manifest_doc(paper_dir, "2605.splitv1", title="Split Fixture")
    package = build_package_from_manifest_document(doc, run_id="test-run")

    rendered = render_review_bundle(package, doc, snippet_chars=120)

    assert ":split-0001" in rendered
    assert ":split-0002" in rendered
    assert "alpha0" in rendered
    assert "beta0" in rendered


def test_generate_review_bundles_writes_summary_reviews_and_events(tmp_path: Path) -> None:
    docs = []
    for paper_id in ["2605.14259v1", "2605.14517v1", "2605.14995v1"]:
        paper_dir = _paper_dir(tmp_path, paper_id, FIXTURE_TEXT)
        docs.append(_manifest_doc(paper_dir, paper_id))
    corpus_path = _write_corpus(tmp_path, docs)

    result = generate_review_bundles(
        corpus_path=corpus_path,
        review_dir=tmp_path / "review",
        events_path=tmp_path / "run-evidence" / "independent-review-events.jsonl",
        run_id="test-run",
    )

    assert result.summary_path.exists()
    assert result.events_path.exists()
    assert len(result.review_paths) >= 3
    for path in result.review_paths:
        assert path.exists()
        assert "Reviewer Checklist" in path.read_text(encoding="utf-8")
        assert "Reviewer Output Contract" in path.read_text(encoding="utf-8")
    events = result.events_path.read_text(encoding="utf-8")
    assert "independent_review.requested" in events
    assert "independent_review.summary" in events
    assert "bounded review fixture" not in events


def test_generated_summary_states_review_is_required_before_eligibility(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.14259v1", FIXTURE_TEXT)
    corpus_path = _write_corpus(tmp_path, [_manifest_doc(paper_dir, "2605.14259v1")])

    result = generate_review_bundles(
        corpus_path=corpus_path,
        review_dir=tmp_path / "review",
        events_path=tmp_path / "run-evidence" / "independent-review-events.jsonl",
        run_id="test-run",
    )

    summary = result.summary_path.read_text(encoding="utf-8")
    assert "Independent reviewer verdicts are still required" in summary
    assert "before route eligibility can be claimed" in summary
    assert "do not accept unreplaced placeholders" in summary


def test_validate_review_artifacts_allows_generated_contracts_before_completion(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.14259v1", FIXTURE_TEXT)
    corpus_path = _write_corpus(tmp_path, [_manifest_doc(paper_dir, "2605.14259v1")])
    result = generate_review_bundles(
        corpus_path=corpus_path,
        review_dir=tmp_path / "review",
        events_path=tmp_path / "run-evidence" / "independent-review-events.jsonl",
        run_id="test-run",
    )

    validation = validate_review_artifacts(review_dir=result.summary_path.parent, events_path=result.events_path)

    assert validation.ok
    assert validation.diagnostics == []


def test_validate_review_artifacts_requires_completed_verdict_when_requested(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.14259v1", FIXTURE_TEXT)
    corpus_path = _write_corpus(tmp_path, [_manifest_doc(paper_dir, "2605.14259v1")])
    result = generate_review_bundles(
        corpus_path=corpus_path,
        review_dir=tmp_path / "review",
        events_path=tmp_path / "run-evidence" / "independent-review-events.jsonl",
        run_id="test-run",
    )

    validation = validate_review_artifacts(
        review_dir=result.summary_path.parent,
        events_path=result.events_path,
        require_completed_review=True,
    )

    assert not validation.ok
    assert any("No independent_review.verdict" in diagnostic for diagnostic in validation.diagnostics)


def test_validate_review_artifacts_accepts_completed_contract_and_rejects_old_placeholder(tmp_path: Path) -> None:
    review_dir = tmp_path / "review"
    review_dir.mkdir()
    events_path = tmp_path / "events.jsonl"
    (review_dir / "paper-review.md").write_text(
        "# Bundle\n\n## Reviewer Output Contract\n\nReturn a completed review result; do not leave placeholders.",
        encoding="utf-8",
    )
    (review_dir / "independent-review-summary.md").write_text(
        "# Summary\n\nverdict: REPAIR\nrepair_required: []\n",
        encoding="utf-8",
    )
    events_path.write_text(
        json.dumps(
            {
                "event": "independent_review.verdict",
                "verdict": "REPAIR",
                "output_contract_completed": True,
            }
        ),
        encoding="utf-8",
    )

    validation = validate_review_artifacts(
        review_dir=review_dir,
        events_path=events_path,
        require_completed_review=True,
    )

    assert validation.ok

    (review_dir / "paper-review.md").write_text("Reviewer Verdict Placeholder", encoding="utf-8")
    validation = validate_review_artifacts(review_dir=review_dir, events_path=events_path)

    assert not validation.ok
    assert any("stale placeholder" in diagnostic for diagnostic in validation.diagnostics)
