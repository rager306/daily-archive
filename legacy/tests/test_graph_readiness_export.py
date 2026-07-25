from __future__ import annotations

import json
from pathlib import Path

from research_graph.infrastructure.graph.readiness.core import (
    ChunkRoute,
    ChunkType,
    GraphReadinessState,
    stable_text_hash,
)
from research_graph.infrastructure.graph.readiness.export import (
    build_package_from_manifest_document,
    export_corpus,
)

FIXTURE_TEXT = """# Test Paper

## Abstract

This paper introduces a traceable graph-readiness fixture.

## Results

The fixture preserves a measurable result with source spans.

## References

[1] Example Reference.
"""

LOW_QUALITY_TEXT = """# Computer Science > Computation and Language

## Submission history

## Access Paper:
"""

METADATA_TEXT = """# Metadata Fixture

## Gang Peng

Huizhou Lateni AI Technology Co., Ltd., Huizhou, China
Huizhou University, Huizhou, China
ORCID: 0009-0007-4774-1681
Correspondence: peng@example.test

## Results

A short scientific result remains available for claim routing.
"""

OVERSIZED_CLAIM_TEXT = """# Oversized Fixture

## Results

{}.
""".format(" ".join(f"token{i}" for i in range(260)))

TABLE_REFERENCE_TEXT = """# Table Fixture

## Table 1

| Metric | Value |
|---|---|
| Accuracy | 0.9 |

## References

[1] Example Reference.
"""

SPLITTABLE_PROSE_TEXT = """# Splittable Fixture

## Results

{}.

{}.

{}.
""".format(
    " ".join(f"alpha{i}" for i in range(70)),
    " ".join(f"beta{i}" for i in range(80)),
    " ".join(f"gamma{i}" for i in range(75)),
)

ADMINISTRATIVE_ROADMAP_TEXT = """# Roadmap Fixture

## Results

The paper is organized as follows. In Section 2 we introduce notation. In Section 3 we present experiments and in Section 4 we conclude.
"""

BACKGROUND_RELATED_WORK_TEXT = """# Background Fixture

## Related Work

Previous work by Smith et al. (2020) and Jones et al. (2021) has been studied in many settings. Prior studies describe similar systems but this passage does not yet establish a new author claim.
"""

MULTI_CLAIM_BUNDLE_TEXT = """# Contribution Fixture

## Results

Our contributions are threefold: we propose a graph-ready validation gate; we demonstrate a source-span audit; we introduce a conservative extraction blocker for noisy claim routes.
"""

UNSPLITTABLE_MULTI_CLAIM_TEXT = """# Unsplit Fixture

## Results

Our contributions are intertwined because we propose a graph-ready validation gate and demonstrate a source-span audit while introducing a conservative extraction blocker in one dependent sentence.
"""


def _paper_dir(
    tmp_path: Path, paper_id: str, text: str | None, *, method: str | None = None
) -> Path:
    paper_dir = tmp_path / "papers" / paper_id
    paper_dir.mkdir(parents=True)
    if text is not None:
        full_text = paper_dir / "full_text.md"
        full_text.write_text(text, encoding="utf-8")
        if method is not None:
            (paper_dir / "full_text.method").write_text(method, encoding="utf-8")
    return paper_dir


def _manifest_doc(paper_dir: Path, paper_id: str) -> dict[str, object]:
    return {
        "rank": 1,
        "paper_id": paper_id,
        "title": f"Fixture {paper_id}",
        "paper_dir": str(paper_dir),
        "expected_full_text_path": str(paper_dir / "full_text.md"),
    }


def _write_corpus(tmp_path: Path, docs: list[dict[str, object]]) -> Path:
    corpus_path = tmp_path / "corpus.json"
    corpus_path.write_text(json.dumps({"documents": docs}), encoding="utf-8")
    return corpus_path


def test_build_package_maps_sections_chunks_and_evidence_paths(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.00001v1", FIXTURE_TEXT, method="docling")

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.00001v1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    assert package.paper_id == "2605.00001v1"
    assert package.report.state == GraphReadinessState.OK_FOR_GRAPH
    assert package.sections
    assert package.chunks
    assert package.evidence_paths
    assert package.chunks[0].text_hash == stable_text_hash(
        "This paper introduces a traceable graph-readiness fixture."
    )
    assert all(chunk.source_span.char_start is not None for chunk in package.chunks)
    assert all(
        path.chunk_id in {chunk.chunk_id for chunk in package.chunks}
        for path in package.evidence_paths
    )


def test_low_quality_source_is_rejected_without_chunks(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.14259v1", LOW_QUALITY_TEXT, method="arxiv2md")

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.14259v1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    assert package.report.state == GraphReadinessState.REJECT
    assert package.chunks == []
    assert any(warning.code == "no_substantive_body" for warning in package.warnings)


def test_deprecated_pymupdf_method_marks_package_repair_required(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.14259v1", FIXTURE_TEXT, method="pymupdf")

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.14259v1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    assert package.report.state == GraphReadinessState.REPAIR_REQUIRED
    assert any(warning.code == "deprecated_pymupdf_cache" for warning in package.warnings)
    assert package.chunks


def test_missing_source_is_rejected_with_typed_warning(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.missingv1", None)

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.missingv1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    assert package.report.state == GraphReadinessState.REJECT
    assert package.chunks == []
    assert any(warning.code == "missing_source" for warning in package.warnings)


def test_export_corpus_writes_redacted_jsonl_and_summary(tmp_path: Path) -> None:
    paper_a = _paper_dir(tmp_path, "2605.00001v1", FIXTURE_TEXT, method="docling")
    paper_b = _paper_dir(tmp_path, "2605.00002v1", LOW_QUALITY_TEXT, method="arxiv2md")
    corpus_path = _write_corpus(
        tmp_path,
        [
            _manifest_doc(paper_a, "2605.00001v1"),
            _manifest_doc(paper_b, "2605.00002v1"),
        ],
    )

    result = export_corpus(corpus_path, tmp_path / "out", run_id="test-run")

    assert result.events_path.exists()
    assert result.summary_path.exists()
    assert result.summary["paper_count"] == 2
    assert result.summary["states"]["ok_for_graph"] == 1
    assert result.summary["states"]["reject"] == 1
    events_text = result.events_path.read_text(encoding="utf-8")
    summary_text = result.summary_path.read_text(encoding="utf-8")
    assert "traceable graph-readiness fixture" not in events_text
    assert "traceable graph-readiness fixture" not in summary_text
    assert "graph_readiness.paper" in events_text
    assert "graph_readiness.route" in events_text


def test_export_is_deterministic_for_same_inputs_and_run_id(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.00001v1", FIXTURE_TEXT, method="docling")
    manifest_doc = _manifest_doc(paper_dir, "2605.00001v1")

    first = build_package_from_manifest_document(
        manifest_doc,
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )
    second = build_package_from_manifest_document(
        manifest_doc,
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    assert first.conversion_id == second.conversion_id
    assert first.document_id == second.document_id
    assert [chunk.chunk_id for chunk in first.chunks] == [chunk.chunk_id for chunk in second.chunks]
    assert [path.evidence_path_id for path in first.evidence_paths] == [
        path.evidence_path_id for path in second.evidence_paths
    ]


def test_author_affiliation_metadata_is_not_claim_extraction(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.14517v1", METADATA_TEXT, method="docling")

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.14517v1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    metadata_chunks = [chunk for chunk in package.chunks if chunk.chunk_type == ChunkType.METADATA]
    assert metadata_chunks
    assert all(ChunkRoute.CLAIM_EXTRACTION not in chunk.routes for chunk in metadata_chunks)
    assert all(ChunkRoute.CLAIM_EXTRACTION in chunk.excluded_routes for chunk in metadata_chunks)


def test_oversized_claim_chunk_is_retrieval_only_until_split(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.oversizedv1", OVERSIZED_CLAIM_TEXT, method="docling")

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.oversizedv1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    result_chunks = [
        chunk
        for chunk in package.chunks
        if "Results" in chunk.section_path[-1] or "results" in chunk.chunk_id
    ]
    assert result_chunks
    assert all(chunk.routes == [ChunkRoute.RETRIEVAL_ONLY] for chunk in result_chunks)
    assert all(
        chunk.quality_state == GraphReadinessState.OK_FOR_RETRIEVAL_ONLY for chunk in result_chunks
    )
    assert any(
        warning.code == "oversized_claim_chunk_retrieval_only"
        for chunk in result_chunks
        for warning in chunk.validation_warnings
    )


def test_splittable_oversized_prose_preserves_source_spans_and_claim_routes(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.splitv1", SPLITTABLE_PROSE_TEXT, method="docling")

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.splitv1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    split_chunks = [chunk for chunk in package.chunks if ":split-" in chunk.chunk_id]
    assert len(split_chunks) == 3
    assert all(ChunkRoute.CLAIM_EXTRACTION in chunk.routes for chunk in split_chunks)
    assert all(chunk.quality_state == GraphReadinessState.OK_FOR_GRAPH for chunk in split_chunks)
    assert all(chunk.parent_chunk_id for chunk in split_chunks)
    assert all(chunk.source_span.char_start is not None for chunk in split_chunks)
    assert all(chunk.source_span.char_end is not None for chunk in split_chunks)
    assert [chunk.order for chunk in split_chunks] == sorted(chunk.order for chunk in split_chunks)
    assert any(
        warning.code == "prose_candidate_split"
        for chunk in split_chunks
        for warning in chunk.validation_warnings
    )
    assert {path.chunk_id for path in package.evidence_paths} >= {
        chunk.chunk_id for chunk in split_chunks
    }


def test_table_and_reference_routes_are_retrieval_only_without_lineage(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.tablev1", TABLE_REFERENCE_TEXT, method="docling")

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.tablev1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    table_chunks = [
        chunk for chunk in package.chunks if chunk.chunk_type == ChunkType.TABLE_CONTEXT
    ]
    reference_chunks = [
        chunk for chunk in package.chunks if chunk.chunk_type == ChunkType.REFERENCE_ENTRY
    ]
    assert table_chunks
    assert reference_chunks
    assert all(ChunkRoute.TABLE_EXTRACTION not in chunk.routes for chunk in table_chunks)
    assert all(ChunkRoute.TABLE_EXTRACTION in chunk.excluded_routes for chunk in table_chunks)
    assert all(ChunkRoute.CITATION_GRAPH not in chunk.routes for chunk in reference_chunks)
    assert all(ChunkRoute.CITATION_GRAPH in chunk.excluded_routes for chunk in reference_chunks)


def test_administrative_roadmap_is_excluded_from_claim_extraction(tmp_path: Path) -> None:
    paper_dir = _paper_dir(
        tmp_path, "2605.roadmapv1", ADMINISTRATIVE_ROADMAP_TEXT, method="docling"
    )

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.roadmapv1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    roadmap_chunks = [
        chunk
        for chunk in package.chunks
        if any(
            warning.code == "administrative_roadmap_not_claim_evidence"
            for warning in chunk.validation_warnings
        )
    ]
    assert roadmap_chunks
    assert all(ChunkRoute.CLAIM_EXTRACTION not in chunk.routes for chunk in roadmap_chunks)
    assert all(ChunkRoute.CLAIM_EXTRACTION in chunk.excluded_routes for chunk in roadmap_chunks)
    assert all(chunk.routes == [ChunkRoute.RETRIEVAL_ONLY] for chunk in roadmap_chunks)
    assert all(
        chunk.quality_state == GraphReadinessState.OK_FOR_RETRIEVAL_ONLY for chunk in roadmap_chunks
    )


def test_related_work_candidate_gets_claim_caveat_without_deleting_text(tmp_path: Path) -> None:
    paper_dir = _paper_dir(
        tmp_path, "2605.backgroundv1", BACKGROUND_RELATED_WORK_TEXT, method="docling"
    )

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.backgroundv1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    related_chunks = [
        chunk
        for chunk in package.chunks
        if any(
            warning.code == "background_related_work_claim_caveat"
            for warning in chunk.validation_warnings
        )
    ]
    assert related_chunks
    assert all(ChunkRoute.CLAIM_EXTRACTION in chunk.routes for chunk in related_chunks)
    assert all(chunk.quality_state == GraphReadinessState.OK_FOR_GRAPH for chunk in related_chunks)
    assert all(chunk.char_count > 0 for chunk in related_chunks)


def test_unsplittable_multi_claim_bundle_is_marked_repair_required_before_extraction(
    tmp_path: Path,
) -> None:
    paper_dir = _paper_dir(
        tmp_path, "2605.unsplitv1", UNSPLITTABLE_MULTI_CLAIM_TEXT, method="docling"
    )

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.unsplitv1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    multi_claim_chunks = [
        chunk
        for chunk in package.chunks
        if any(
            warning.code == "multi_claim_candidate_requires_atomic_split"
            for warning in chunk.validation_warnings
        )
    ]
    assert multi_claim_chunks
    assert all(ChunkRoute.CLAIM_EXTRACTION in chunk.routes for chunk in multi_claim_chunks)
    assert all(
        chunk.quality_state == GraphReadinessState.REPAIR_REQUIRED for chunk in multi_claim_chunks
    )


def test_semicolon_contribution_bundle_splits_into_atomic_claim_candidates(tmp_path: Path) -> None:
    paper_dir = _paper_dir(tmp_path, "2605.atomicv1", MULTI_CLAIM_BUNDLE_TEXT, method="docling")

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.atomicv1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    atomic_chunks = [
        chunk
        for chunk in package.chunks
        if any(
            warning.code == "atomic_claim_candidate_split" for warning in chunk.validation_warnings
        )
    ]
    assert len(atomic_chunks) == 3
    assert all(ChunkRoute.CLAIM_EXTRACTION in chunk.routes for chunk in atomic_chunks)
    assert all(chunk.quality_state == GraphReadinessState.OK_FOR_GRAPH for chunk in atomic_chunks)
    assert all(chunk.parent_chunk_id for chunk in atomic_chunks)
    assert all(chunk.source_span.char_start is not None for chunk in atomic_chunks)
    assert all(chunk.source_span.char_end is not None for chunk in atomic_chunks)
    assert [chunk.order for chunk in atomic_chunks] == sorted(
        chunk.order for chunk in atomic_chunks
    )


def test_single_claim_prose_is_not_over_split(tmp_path: Path) -> None:
    paper_dir = _paper_dir(
        tmp_path,
        "2605.singleclaimv1",
        """# Single Claim Fixture

## Results

We demonstrate that the graph-readiness gate preserves source spans for one measured extraction result.
""",
        method="docling",
    )

    package = build_package_from_manifest_document(
        _manifest_doc(paper_dir, "2605.singleclaimv1"),
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
    )

    assert len(package.chunks) == 1
    assert not any(
        warning.code
        in {"atomic_claim_candidate_split", "multi_claim_candidate_requires_atomic_split"}
        for chunk in package.chunks
        for warning in chunk.validation_warnings
    )
