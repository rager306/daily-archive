from __future__ import annotations

from research_graph.graph.readiness.core import (
    ChunkRoute,
    ChunkType,
    CoordinateSpace,
    EvidencePathRef,
    ExtractionTrustLevel,
    GraphReadinessReport,
    GraphReadinessState,
    GraphReadyChunk,
    NormalizedPaperPackage,
    QualityWarning,
    SourceSpan,
    WarningSeverity,
    stable_text_hash,
    to_redacted_dict,
    validate_graph_ready_chunk,
    validate_normalized_package,
)


def _span() -> SourceSpan:
    return SourceSpan(
        source_path="/tmp/full_text.md",
        coordinate_space=CoordinateSpace.NORMALIZED_MARKDOWN_CHAR,
        char_start=10,
        char_end=50,
        line_start=2,
        line_end=4,
        span_confidence=1.0,
    )


def _chunk(*, routes: list[ChunkRoute] | None = None, source_span: SourceSpan | None = None) -> GraphReadyChunk:
    return GraphReadyChunk(
        chunk_id="2605.00001v1:introduction:chunk-0001",
        paper_id="2605.00001v1",
        parent_element_ids=["section:introduction", "paragraph:introduction:1"],
        section_path=["Introduction"],
        order=0,
        chunk_type=ChunkType.CLAIM_CANDIDATE,
        routes=routes if routes is not None else [ChunkRoute.CLAIM_EXTRACTION],
        excluded_routes=[ChunkRoute.CITATION_GRAPH],
        source_span=source_span or _span(),
        text_hash=stable_text_hash("A bounded fixture claim."),
        char_count=24,
        chunking_strategy="fixture",
        chunking_version="fixture.v1",
        quality_state=GraphReadinessState.OK_FOR_GRAPH,
        provenance={"paper_id": "2605.00001v1"},
    )


def _evidence_path(chunk: GraphReadyChunk | None = None) -> EvidencePathRef:
    chunk = chunk or _chunk()
    return EvidencePathRef(
        evidence_path_id="evidence:2605.00001v1:introduction:chunk-0001",
        paper_id="2605.00001v1",
        conversion_id="conversion:2605.00001v1:docling",
        document_id="document:2605.00001v1:normalized",
        source_element_ids=["section:introduction", "paragraph:introduction:1"],
        chunk_id=chunk.chunk_id,
        section_path=list(chunk.section_path),
        source_spans=[chunk.source_span],
        route=ChunkRoute.CLAIM_EXTRACTION,
        quality_state=GraphReadinessState.OK_FOR_GRAPH,
    )


def _report() -> GraphReadinessReport:
    return GraphReadinessReport(
        run_id="test-run",
        paper_id="2605.00001v1",
        conversion_id="conversion:2605.00001v1:docling",
        document_id="document:2605.00001v1:normalized",
        state=GraphReadinessState.OK_FOR_GRAPH,
        trust_level=ExtractionTrustLevel.TRUSTED_GRAPH,
        counts={"chunks": 1, "evidence_paths": 1},
        coverage={"chunks_with_char_span_rate": 1.0},
        routes={"claim_extraction": {"eligible": 1, "blocked": 0}},
    )


def _package(*, chunk: GraphReadyChunk | None = None, evidence_path: EvidencePathRef | None = None) -> NormalizedPaperPackage:
    chunk = chunk or _chunk()
    evidence_path = evidence_path or _evidence_path(chunk)
    return NormalizedPaperPackage(
        contract_version="graph-ready-data-contract.v1",
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
        schema_version="graph-readiness.schema.v1",
        normalizer_version="fixture-normalizer.v1",
        paper_id="2605.00001v1",
        conversion_id="conversion:2605.00001v1:docling",
        document_id="document:2605.00001v1:normalized",
        chunks=[chunk],
        evidence_paths=[evidence_path],
        report=_report(),
        sections=[{"section_id": "section:introduction", "paper_id": "2605.00001v1"}],
        paragraphs=[{"paragraph_id": "paragraph:introduction:1", "paper_id": "2605.00001v1"}],
    )


def test_valid_package_passes_contract_validation() -> None:
    result = validate_normalized_package(_package())

    assert result.ok is True
    assert result.blockers == []


def test_missing_source_span_blocks_graph_ready_chunk() -> None:
    bad_span = SourceSpan(
        source_path="/tmp/full_text.md",
        coordinate_space=CoordinateSpace.ELEMENT_LOCAL_CHAR,
        char_start=0,
        char_end=25,
        span_confidence=1.0,
    )
    warnings = validate_graph_ready_chunk(_chunk(source_span=bad_span))

    assert any(warning.code == "missing_or_untrusted_source_span" for warning in warnings)
    assert any(warning.severity == WarningSeverity.BLOCKER for warning in warnings)


def test_missing_route_blocks_extraction_eligibility() -> None:
    warnings = validate_graph_ready_chunk(_chunk(routes=[]))

    assert any(warning.code == "missing_chunk_route" for warning in warnings)
    assert any(warning.severity == WarningSeverity.BLOCKER for warning in warnings)


def test_unresolved_evidence_path_blocks_package() -> None:
    evidence_path = _evidence_path()
    evidence_path = EvidencePathRef(
        evidence_path_id=evidence_path.evidence_path_id,
        paper_id=evidence_path.paper_id,
        conversion_id=evidence_path.conversion_id,
        document_id=evidence_path.document_id,
        source_element_ids=evidence_path.source_element_ids,
        chunk_id="missing-chunk",
        section_path=evidence_path.section_path,
        source_spans=evidence_path.source_spans,
        route=evidence_path.route,
    )

    result = validate_normalized_package(_package(evidence_path=evidence_path))

    assert result.ok is False
    assert any(warning.code == "evidence_path_missing_chunk" for warning in result.blockers)


def test_claim_route_rejects_reference_chunk_type() -> None:
    chunk = GraphReadyChunk(
        chunk_id="2605.00001v1:references:chunk-0001",
        paper_id="2605.00001v1",
        parent_element_ids=["section:references"],
        section_path=["References"],
        order=2,
        chunk_type=ChunkType.REFERENCE_ENTRY,
        routes=[ChunkRoute.CLAIM_EXTRACTION],
        source_span=_span(),
        text_hash=stable_text_hash("Reference entry."),
        char_count=16,
    )

    warnings = validate_graph_ready_chunk(chunk)

    assert any(warning.code == "claim_route_for_non_claim_chunk" for warning in warnings)


def test_rejected_chunk_state_blocks_package() -> None:
    chunk = GraphReadyChunk(
        chunk_id="2605.00001v1:introduction:chunk-0001",
        paper_id="2605.00001v1",
        parent_element_ids=["section:introduction", "paragraph:introduction:1"],
        section_path=["Introduction"],
        order=0,
        chunk_type=ChunkType.CLAIM_CANDIDATE,
        routes=[ChunkRoute.CLAIM_EXTRACTION],
        source_span=_span(),
        text_hash=stable_text_hash("Rejected chunk."),
        char_count=15,
        quality_state=GraphReadinessState.REJECT,
    )

    result = validate_normalized_package(_package(chunk=chunk, evidence_path=_evidence_path(chunk)))

    assert result.ok is False
    assert any(warning.code == "chunk_state_blocks_extraction" for warning in result.blockers)


def test_redacted_serialization_hashes_raw_text_fields() -> None:
    payload = {
        "paper_id": "2605.00001v1",
        "chunk_text": "This raw chunk must not be logged.",
        "nested": {"prompt": "do not log prompts"},
    }

    redacted = to_redacted_dict(payload)

    assert redacted["paper_id"] == "2605.00001v1"
    assert redacted["chunk_text"]["sha256"] == stable_text_hash("This raw chunk must not be logged.")
    assert redacted["chunk_text"]["length"] == len("This raw chunk must not be logged.")
    assert redacted["nested"]["prompt"]["sha256"] == stable_text_hash("do not log prompts")
    assert "This raw chunk" not in str(redacted)


def test_package_warning_blocker_is_not_silently_ignored() -> None:
    warning = QualityWarning(
        code="wrong_paper_identity",
        severity=WarningSeverity.BLOCKER,
        message="Converted artifact did not match the target paper.",
        object_type="Paper",
        object_id="2605.00001v1",
    )
    package = NormalizedPaperPackage(
        contract_version="graph-ready-data-contract.v1",
        run_id="test-run",
        created_at="2026-05-18T00:00:00Z",
        schema_version="graph-readiness.schema.v1",
        normalizer_version="fixture-normalizer.v1",
        paper_id="2605.00001v1",
        conversion_id="conversion:2605.00001v1:docling",
        document_id="document:2605.00001v1:normalized",
        chunks=[_chunk()],
        evidence_paths=[_evidence_path()],
        report=_report(),
        sections=[{"section_id": "section:introduction"}],
        paragraphs=[{"paragraph_id": "paragraph:introduction:1"}],
        warnings=[warning],
    )

    result = validate_normalized_package(package)

    assert result.ok is False
    assert any(warning.code == "wrong_paper_identity" for warning in result.blockers)
