from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.chunking_benchmark import (
    ChunkingBenchmark,
    MethodMetrics,
    PaperMethodMetrics,
    aggregate_method_metrics,
    build_benchmark_from_artifacts,
    method_from_baseline_summary,
    method_from_simple_section_window,
    method_from_structure_aware_summary,
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


def test_method_from_baseline_summary_maps_retrieval_only_baseline() -> None:
    method = method_from_baseline_summary(
        {
            "paper_count": 2,
            "refused_chunk_count": 7,
            "import_eligible_chunk_count": 0,
            "counts_by_route": {"retrieval_only": 7},
            "counts_by_chunk_type": {"retrieval_context": 7},
            "counts_by_state": {"ok_for_retrieval_only": 7},
            "refusal_counts": {"baseline_retrieval_only_not_import_ready": 7},
        }
    ).to_contract()

    assert method["method_id"] == "baseline_pageindex_semanticchunk"
    assert method["paper_count"] == 2
    assert method["chunk_count"] == 7
    assert method["asset_linkage_coverage_rate"] == 0.0
    assert "no_annotation_or_asset_linkage" in method["caveats"]


def test_method_from_structure_aware_summary_uses_annotation_and_asset_coverage() -> None:
    method = method_from_structure_aware_summary(
        {
            "paper_count": 2,
            "chunk_count": 10,
            "import_eligible_chunk_count": 0,
            "refused_chunk_count": 10,
            "counts_by_route": {"claim_extraction": 4, "retrieval_only": 6},
            "counts_by_chunk_type": {"claim_candidate": 4, "retrieval_context": 6},
            "counts_by_state": {"repair_required": 4, "ok_for_retrieval_only": 6},
            "refusal_counts": {"claim_route_requires_review": 4, "retrieval_only_not_import_ready": 6},
        },
        annotation_summary={"annotated_chunk_count": 10},
        source_asset_summary={"asset_count": 2, "missing_counts": {"missing_original_pdf": 1}},
    ).to_contract()

    assert method["method_id"] == "structure_aware_control"
    assert method["source_span_coverage"] == 1.0
    assert method["parent_reference_resolution_rate"] == 1.0
    assert method["annotation_coverage_rate"] == 1.0
    assert method["asset_linkage_coverage_rate"] == 0.2
    assert method["missing_source_counts"] == {"missing_original_pdf": 1}


def test_method_from_simple_section_window_estimate_stays_import_blocked() -> None:
    method = method_from_simple_section_window(
        {
            "paper_count": 2,
            "source_file_count": 3,
            "asset_count": 6,
            "asset_counts_by_type": {"table": 2, "figure": 1, "equation": 1, "reference": 1, "metadata": 1},
            "missing_counts": {"missing_original_pdf": 1},
        }
    ).to_contract()

    assert method["method_id"] == "simple_section_window_estimate"
    assert method["chunk_count"] == 9
    assert method["refused_chunk_count"] == 9
    assert method["import_eligible_chunk_count"] == 0
    assert method["asset_linkage_coverage_rate"] == 6 / 9
    assert "chonkie_llamaindex_langchain_not_executed" in method["caveats"]


def test_build_benchmark_from_artifacts_reads_redacted_summaries(tmp_path: Path) -> None:
    baseline = tmp_path / "baseline.json"
    structure = tmp_path / "structure.json"
    annotations = tmp_path / "annotations.json"
    source_assets = tmp_path / "source-assets.json"
    baseline.write_text(
        json.dumps(
            {
                "paper_count": 1,
                "refused_chunk_count": 2,
                "import_eligible_chunk_count": 0,
                "counts_by_route": {"retrieval_only": 2},
                "counts_by_chunk_type": {"retrieval_context": 2},
                "counts_by_state": {"ok_for_retrieval_only": 2},
                "refusal_counts": {"baseline_retrieval_only_not_import_ready": 2},
            }
        ),
        encoding="utf-8",
    )
    structure.write_text(
        json.dumps(
            {
                "paper_count": 1,
                "chunk_count": 4,
                "import_eligible_chunk_count": 0,
                "refused_chunk_count": 4,
                "counts_by_route": {"table_extraction": 1, "retrieval_only": 3},
                "counts_by_chunk_type": {"table_context": 1, "retrieval_context": 3},
                "counts_by_state": {"repair_required": 1, "ok_for_retrieval_only": 3},
                "refusal_counts": {"table_route_requires_review": 1, "retrieval_only_not_import_ready": 3},
            }
        ),
        encoding="utf-8",
    )
    annotations.write_text(json.dumps({"annotated_chunk_count": 4}), encoding="utf-8")
    source_assets.write_text(
        json.dumps(
            {
                "paper_count": 1,
                "source_file_count": 1,
                "asset_count": 1,
                "asset_counts_by_type": {"table": 1},
                "missing_counts": {},
            }
        ),
        encoding="utf-8",
    )

    benchmark = build_benchmark_from_artifacts(
        input_corpus="gold",
        baseline_summary_path=baseline,
        structure_summary_path=structure,
        annotation_summary_path=annotations,
        source_asset_summary_path=source_assets,
    ).to_contract()

    assert validate_chunking_benchmark(benchmark).valid_benchmark is True
    assert benchmark["method_count"] == 3
    assert benchmark["aggregate"]["method_ids"] == [
        "baseline_pageindex_semanticchunk",
        "simple_section_window_estimate",
        "structure_aware_control",
    ]
    assert benchmark["recommendation_status"] == "review_required"
    assert "real_library_candidates_not_executed" in benchmark["caveats"]
