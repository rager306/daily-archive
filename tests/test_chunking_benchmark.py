from __future__ import annotations

import json

from arxiv_archive.chunking_benchmark import (
    ChunkingBenchmark,
    MethodMetrics,
    PaperMethodMetrics,
    aggregate_method_metrics,
    validate_chunking_benchmark,
)


def _method(method_id: str = "structure_aware") -> MethodMetrics:
    return MethodMetrics(
        method_id=method_id,
        paper_count=2,
        chunk_count=10,
        import_eligible_chunk_count=0,
        refused_chunk_count=10,
        counts_by_route={"claim_extraction": 3, "retrieval_only": 7},
        counts_by_chunk_type={"claim_candidate": 3, "retrieval_context": 7},
        counts_by_state={"repair_required": 3, "ok_for_retrieval_only": 7},
        refusal_counts={"claim_route_requires_review": 3, "retrieval_only_not_import_ready": 7},
        source_span_coverage=1.0,
        parent_reference_resolution_rate=1.0,
        annotation_coverage_rate=1.0,
        asset_linkage_coverage_rate=0.2,
        missing_source_counts={"missing_original_pdf": 1},
        caveats=("dry_run_only",),
    )


def test_method_metrics_serializes_redacted_counts_and_flags() -> None:
    record = _method().to_contract()

    assert record["method_id"] == "structure_aware"
    assert record["chunk_count"] == 10
    assert record["counts_by_route"] == {"claim_extraction": 3, "retrieval_only": 7}
    assert record["source_span_coverage"] == 1.0
    assert record["annotation_coverage_rate"] == 1.0
    assert record["asset_linkage_coverage_rate"] == 0.2
    assert record["missing_source_counts"] == {"missing_original_pdf": 1}
    assert record["raw_text_included"] is False
    assert record["chunk_text_included"] is False
    assert record["embeddings_included"] is False
    assert record["vectors_included"] is False
    assert record["ladybugdb_written"] is False
    assert record["production_import_attempted"] is False
    assert "raw chunk text" not in json.dumps(record)


def test_paper_method_metrics_serializes_per_paper_asset_linkage() -> None:
    record = PaperMethodMetrics(
        paper_id="p1",
        method_id="structure_aware",
        chunk_count=4,
        refused_chunk_count=4,
        asset_count=2,
        counts_by_route={"table_extraction": 1, "retrieval_only": 3},
        asset_linkage_coverage_rate=0.5,
    ).to_contract()

    assert record["paper_id"] == "p1"
    assert record["asset_count"] == 2
    assert record["asset_linkage_coverage_rate"] == 0.5
    assert record["raw_text_included"] is False


def test_chunking_benchmark_contract_aggregates_methods() -> None:
    benchmark = ChunkingBenchmark(
        input_corpus="gold-corpus",
        methods=(
            _method("baseline"),
            MethodMetrics(
                method_id="structure_aware",
                paper_count=2,
                chunk_count=20,
                refused_chunk_count=20,
                counts_by_route={"table_extraction": 2},
                counts_by_chunk_type={"table_context": 2},
                counts_by_state={"repair_required": 2},
                refusal_counts={"table_route_requires_review": 2},
                source_span_coverage=1.0,
                annotation_coverage_rate=1.0,
                asset_linkage_coverage_rate=0.1,
            ),
        ),
        per_paper=(PaperMethodMetrics(paper_id="p1", method_id="baseline", chunk_count=10, refused_chunk_count=10),),
        recommendation_status="review_required",
        caveats=("no_method_import_ready",),
    ).to_contract()
    validation = validate_chunking_benchmark(benchmark)

    assert validation.valid_benchmark is True
    assert benchmark["method_count"] == 2
    assert benchmark["aggregate"]["total_chunk_count"] == 30
    assert benchmark["aggregate"]["total_refused_chunk_count"] == 30
    assert benchmark["aggregate"]["counts_by_route"] == {
        "claim_extraction": 3,
        "retrieval_only": 7,
        "table_extraction": 2,
    }
    assert benchmark["aggregate"]["method_ids"] == ["baseline", "structure_aware"]
    assert benchmark["raw_text_included"] is False


def test_aggregate_method_metrics_merges_counts_and_missing_sources() -> None:
    aggregate = aggregate_method_metrics(
        [
            _method("a").to_contract(),
            MethodMetrics(
                method_id="b",
                paper_count=1,
                chunk_count=5,
                refused_chunk_count=5,
                counts_by_route={"retrieval_only": 5},
                missing_source_counts={"missing_original_pdf": 2},
            ).to_contract(),
        ]
    )

    assert aggregate["total_chunk_count"] == 15
    assert aggregate["total_refused_chunk_count"] == 15
    assert aggregate["counts_by_route"] == {"claim_extraction": 3, "retrieval_only": 12}
    assert aggregate["missing_source_counts"] == {"missing_original_pdf": 3}


def test_validate_benchmark_rejects_missing_method_fields_and_bad_ranges() -> None:
    benchmark = ChunkingBenchmark(input_corpus="gold", methods=(_method(),)).to_contract()
    del benchmark["methods"][0]["chunk_count"]
    benchmark["methods"][0]["source_span_coverage"] = 1.5

    validation = validate_chunking_benchmark(benchmark)

    assert validation.valid_benchmark is False
    assert validation.refusal_counts["missing_chunk_count"] == 1
    assert validation.refusal_counts["invalid_source_span_coverage"] == 1


def test_validate_benchmark_rejects_nested_raw_text_or_embedding_leakage() -> None:
    leaked = "do not echo this raw chunk"
    benchmark = ChunkingBenchmark(input_corpus="gold", methods=(_method(),)).to_contract()
    benchmark["methods"][0]["caveats"].append({"raw_text": leaked})
    benchmark["per_paper"].append({"paper_id": "p1", "method_id": "m", "chunk_count": 1, "import_eligible_chunk_count": 0, "refused_chunk_count": 1, "embedding": [0.1]})

    validation = validate_chunking_benchmark(benchmark)

    assert validation.valid_benchmark is False
    assert validation.refusal_counts["raw_text_leakage"] >= 1
    assert validation.refusal_counts["embedding_leakage"] >= 1
    serialized = json.dumps([diagnostic.__dict__ for diagnostic in validation.diagnostics])
    assert leaked not in serialized
    assert "0.1" not in serialized


def test_validate_benchmark_rejects_unsafe_flags() -> None:
    benchmark = ChunkingBenchmark(input_corpus="gold", methods=(_method(),)).to_contract()
    benchmark["production_import_attempted"] = True
    benchmark["methods"][0]["embeddings_included"] = True

    validation = validate_chunking_benchmark(benchmark)

    assert validation.valid_benchmark is False
    assert validation.refusal_counts["unsafe_production_import_attempted"] == 1
    assert validation.refusal_counts["unsafe_embeddings_included"] >= 1
