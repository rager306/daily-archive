from __future__ import annotations

import json
from pathlib import Path

from research_graph.workflows.import_boundary_rehearsal import (
    SCHEMA_VERSION,
    ImportBoundaryRehearsal,
    ImportCandidate,
    build_import_boundary_rehearsal_from_benchmark,
    validate_import_boundary_rehearsal,
    write_import_boundary_rehearsal_run,
)

# Stable post flat-phase path (was .gsd/milestones/M005-.../run-evidence/).
M005_CHUNKING_BENCHMARK_DIR = (
    Path(__file__).resolve().parents[1] / "artifacts" / "m005-chunking-benchmark"
)
M005_CHUNKING_SUMMARY = M005_CHUNKING_BENCHMARK_DIR / "chunking-benchmark-summary.json"
M005_CHUNKING_DIAGNOSTICS = (
    M005_CHUNKING_BENCHMARK_DIR / "chunking-benchmark-diagnostics.jsonl"
)


def _candidate(**overrides: object) -> ImportCandidate:
    values = {
        "candidate_id": "candidate-1",
        "method_id": "structure_aware_control",
        "package_id": "2605.14259v1",
        "candidate_type": "claim_candidate",
        "route": "claim_extraction",
        "state": "repair_required",
        "import_eligible": False,
        "refusal_reasons": ("not_reviewed_for_trusted_import",),
        "remediation_hints": ("review_candidate_and_evidence_span",),
    }
    values.update(overrides)  # pyrefly: ignore [bad-assignment, no-matching-overload]
    return ImportCandidate(**values)


def _rehearsal(*candidates: ImportCandidate) -> dict[str, object]:
    return ImportBoundaryRehearsal(
        rehearsal_id="m005-s07-negative-import-boundary",
        source_benchmark_id="m005-s06-chunking-benchmark",
        candidates=candidates or (_candidate(),),
        remediation_hints=("create_reviewed_import_eligible_subset",),
        caveats=("negative_rehearsal_only",),
    ).to_contract()


def test_import_boundary_rehearsal_serializes_negative_candidate() -> None:
    contract = _rehearsal()

    assert contract["schema_version"] == SCHEMA_VERSION
    assert contract["candidate_count"] == 1
    assert contract["accepted_count"] == 0
    assert contract["rejected_count"] == 1
    assert contract["refusal_counts"] == {"not_reviewed_for_trusted_import": 1}
    assert contract["recommendation"] == "positive_import_blocked"
    candidate = contract["candidates"][0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    assert candidate["accepted"] is False
    assert candidate["rejected"] is True
    assert candidate["import_eligible"] is False
    assert "trusted_kg_import" not in candidate["allowed_uses"]
    assert "trusted_kg_import" in candidate["excluded_uses"]
    assert candidate["production_import_attempted"] is False
    assert candidate["ladybugdb_written"] is False


def test_validate_import_boundary_rehearsal_accepts_redacted_negative_contract() -> None:
    validation = validate_import_boundary_rehearsal(_rehearsal())

    assert validation.valid_rehearsal is True
    assert validation.passed is True
    assert validation.diagnostics == ()


def test_validate_import_boundary_rehearsal_rejects_count_mismatch() -> None:
    contract = _rehearsal()
    contract["accepted_count"] = 1
    contract["refusal_counts"] = {}

    validation = validate_import_boundary_rehearsal(contract)

    assert validation.valid_rehearsal is False
    assert validation.refusal_counts["accepted_count_mismatch"] == 1
    assert validation.refusal_counts["refusal_counts_mismatch"] == 1


def test_validate_import_boundary_rehearsal_rejects_positive_import_for_refused_candidate() -> None:
    contract = _rehearsal()
    candidate = contract["candidates"][0]  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
    candidate["accepted"] = True
    candidate["rejected"] = False
    candidate["import_eligible"] = False
    candidate["allowed_uses"] = ["trusted_kg_import"]
    candidate["excluded_uses"] = []

    validation = validate_import_boundary_rehearsal(contract)

    assert validation.valid_rehearsal is False
    assert validation.refusal_counts["accepted_count_mismatch"] == 1
    assert validation.refusal_counts["accepted_candidate_not_import_eligible"] == 1
    assert validation.refusal_counts["candidate_allows_trusted_import"] == 1
    assert validation.refusal_counts["candidate_missing_import_exclusion"] == 1


def test_validate_import_boundary_rehearsal_rejects_unsafe_write_flags() -> None:
    contract = _rehearsal()
    contract["production_import_attempted"] = True
    contract["ladybugdb_written"] = True
    contract["candidates"][0]["embeddings_included"] = True  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]

    validation = validate_import_boundary_rehearsal(contract)

    assert validation.valid_rehearsal is False
    assert validation.refusal_counts["unsafe_production_import_attempted"] == 1
    assert validation.refusal_counts["unsafe_ladybugdb_written"] == 1
    assert validation.refusal_counts["unsafe_embeddings_included"] == 1


def test_validate_import_boundary_rehearsal_rejects_nested_forbidden_fields_without_values() -> (
    None
):
    contract = _rehearsal()
    contract["candidates"][0]["diagnostic"] = {  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[not-subscriptable]
        "raw_text": "do not expose me",
        "embedding": [0.1, 0.2],
        "vector": [0.3],
        "secret": "token-value",
        "optimizer_trace": "trace-value",
    }

    validation = validate_import_boundary_rehearsal(contract)

    assert validation.valid_rehearsal is False
    assert validation.refusal_counts["raw_text_leakage"] == 1
    assert validation.refusal_counts["embedding_leakage"] == 1
    assert validation.refusal_counts["vector_leakage"] == 1
    assert validation.refusal_counts["secret_leakage"] == 1
    assert validation.refusal_counts["optimizer_trace_leakage"] == 1
    assert all(
        "do not expose me" not in (diagnostic.object_id or "")
        for diagnostic in validation.diagnostics
    )


def test_build_import_boundary_rehearsal_from_current_benchmark_artifacts() -> None:
    contract = build_import_boundary_rehearsal_from_benchmark(
        summary_path=M005_CHUNKING_SUMMARY,
        diagnostics_path=M005_CHUNKING_DIAGNOSTICS,
    )

    validation = validate_import_boundary_rehearsal(contract)

    assert validation.valid_rehearsal is True
    assert contract["candidate_count"] == 2471
    assert contract["accepted_count"] == 0
    assert contract["rejected_count"] == 2471
    assert contract["source_benchmark_summary"]["total_chunk_count"] == 2471
    assert contract["source_benchmark_summary"]["total_import_eligible_chunk_count"] == 0
    assert contract["source_benchmark_summary"]["recommendation_status"] == "review_required"
    assert set(contract["refusal_counts"]) == {
        "administrative_metadata_requires_review",
        "baseline_retrieval_only_not_import_ready",
        "citation_route_requires_review",
        "claim_route_requires_review",
        "equation_route_not_import_ready",
        "estimated_candidate_requires_review",
        "figure_route_not_import_ready",
        "method_route_requires_review",
        "retrieval_only_not_import_ready",
        "table_route_requires_review",
    }
    assert all(candidate["accepted"] is False for candidate in contract["candidates"])
    assert all(
        candidate["production_import_attempted"] is False for candidate in contract["candidates"]
    )
    assert all(
        "trusted_kg_import" in candidate["excluded_uses"] for candidate in contract["candidates"]
    )


def test_build_import_boundary_rehearsal_preserves_missing_source_caveats() -> None:
    contract = build_import_boundary_rehearsal_from_benchmark(
        summary_path=M005_CHUNKING_SUMMARY,
        diagnostics_path=M005_CHUNKING_DIAGNOSTICS,
    )

    assert "missing_original_pdf:16" in contract["caveats"]


def test_write_import_boundary_rehearsal_run_writes_summary_and_diagnostics(tmp_path: Path) -> None:
    paths = write_import_boundary_rehearsal_run(
        summary_path=M005_CHUNKING_SUMMARY,
        diagnostics_path=M005_CHUNKING_DIAGNOSTICS,
        output_dir=tmp_path,
    )

    summary = json.loads(paths["summary_path"].read_text(encoding="utf-8"))
    diagnostics = [
        json.loads(line)
        for line in paths["diagnostics_path"].read_text(encoding="utf-8").splitlines()
    ]

    assert paths["summary_path"].name == "import-boundary-summary.json"
    assert paths["diagnostics_path"].name == "import-boundary-diagnostics.jsonl"
    assert "candidates" not in summary
    assert summary["candidate_count"] == 2471
    assert summary["accepted_count"] == 0
    assert summary["rejected_count"] == 2471
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert len(diagnostics) == 2471
    assert diagnostics[0]["accepted"] is False
    assert diagnostics[0]["rejected"] is True
    assert diagnostics[0]["raw_text_included"] is False


def test_validate_import_boundary_rehearsal_requires_rejected_candidate_refusal_reason() -> None:
    contract = _rehearsal(_candidate(refusal_reasons=()))

    validation = validate_import_boundary_rehearsal(contract)

    assert validation.valid_rehearsal is False
    assert validation.refusal_counts["rejected_candidate_missing_refusal"] == 1
