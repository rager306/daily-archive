"""Contract tests for metadata-only article retrieval/table benchmarks.

S06 defines deterministic CPU-only benchmark manifests for section-aware
retrieval units and table transformation candidates. The contract is intentionally
metadata-only: rankings are computed from IDs, counters, hashes, PageIndex/assets
/links provenance, and stable tie-breakers, never from article prose, captions,
table contents, embeddings, vectors, DSPy/RLM output, optimizer traces, graph
imports, or production writes.
"""

from __future__ import annotations

import importlib
import json
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_retrieval_tables"

FORBIDDEN_SENTINELS = (
    "FORBIDDEN_RAW_ARTICLE_TEXT_DO_NOT_ECHO",
    "FORBIDDEN_TABLE_TEXT_DO_NOT_ECHO",
    "FORBIDDEN_CAPTION_TEXT_DO_NOT_ECHO",
    "secret-token",
    "api_key=",
    "token=",
    "embedding=[",
    "vector=[",
)
FORBIDDEN_EXACT_KEYS = {
    "text",
    "raw_text",
    "chunk_text",
    "paper_text",
    "claim_text",
    "section_text",
    "caption_text",
    "table_text",
    "equation_text",
    "model_output",
    "raw_model_output",
    "raw_minimax_response",
    "base64",
    "binary",
    "bytes",
    "image_bytes",
    "payload",
    "embedding",
    "embeddings",
    "vector",
    "vectors",
    "secret",
    "secrets",
    "token",
    "tokens",
    "api_key",
    "credentials",
    "optimizer_trace",
    "optimizer_traces",
    "source_of_truth",
}
UNSAFE_TRUE_FRAGMENTS = (
    '"trusted_kg_import_allowed": true',
    '"ladybugdb_written": true',
    '"production_import_attempted": true',
    '"graph_import_claim": true',
    '"raw_payloads_included": true',
    '"table_text_included": true',
    '"caption_text_included": true',
    '"embeddings_included": true',
    '"vectors_included": true',
    '"dspy_used": true',
    '"rlm_used": true',
    '"optimizer_used": true',
    '"import_eligible": true',
    '"promoted_to_fact": true',
)
EXPECTED_DIAGNOSTIC_COUNTER_KEYS = {
    "duplicate_id_count",
    "malformed_source_ref_count",
    "missing_page_index_provenance_count",
    "missing_asset_provenance_count",
    "bad_vocabulary_count",
    "forbidden_payload_detection_count",
    "unsafe_authorization_count",
    "unsafe_readiness_count",
}
EXPECTED_VALID_STATUSES = {
    "included_review_only",
    "blocked_review_only",
    "repair_required_review_only",
    "excluded_review_only",
}


@pytest.fixture()
def retrieval_tables_contract():
    """Load the future S06 implementation without failing collection.

    T01 is contract-first. Until ``research_graph.infrastructure.papers.indexing.retrieval_tables``
    exists, pytest reports these tests as expected failures rather than an
    import-time collection error. Once the module lands, the contract runs
    normally.
    """

    try:
        return importlib.import_module(
            "research_graph.infrastructure.papers.indexing.retrieval_tables"
        )
    except ModuleNotFoundError as exc:
        if exc.name == "research_graph.infrastructure.papers.indexing.retrieval_tables":
            pytest.xfail(
                "research_graph.infrastructure.papers.indexing.retrieval_tables is not implemented yet"
            )
        raise


def _load_fixture(name: str) -> dict[str, Any]:
    return json.loads((FIXTURES_DIR / name).read_text(encoding="utf-8"))


def _walk_keys(value: object) -> list[str]:
    if isinstance(value, dict):
        keys = list(value.keys())
        for child in value.values():
            keys.extend(_walk_keys(child))
        return keys
    if isinstance(value, list):
        keys: list[str] = []
        for child in value:
            keys.extend(_walk_keys(child))
        return keys
    return []


def _diagnostic_codes(manifest: dict[str, Any]) -> set[str]:
    return {str(diagnostic["code"]) for diagnostic in manifest.get("diagnostics", [])}


def _diagnostic_paths(manifest: dict[str, Any]) -> set[str]:
    return {str(diagnostic["json_path"]) for diagnostic in manifest.get("diagnostics", [])}


def _assert_metadata_only(payload: dict[str, Any] | list[dict[str, Any]]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    for sentinel in FORBIDDEN_SENTINELS:
        assert sentinel not in serialized
    for fragment in UNSAFE_TRUE_FRAGMENTS:
        assert fragment not in serialized
    assert not (set(_walk_keys(payload)) & FORBIDDEN_EXACT_KEYS)


def _builder_kwargs(manifest: dict[str, Any]) -> dict[str, Any]:
    return {
        "paper_id": manifest["paper_id"],
        "run_id": manifest["run_id"],
        "source_refs": manifest["source_refs"],
        "page_index_refs": manifest["page_index_refs"],
        "asset_refs": manifest["asset_refs"],
        "links_dedup_refs": manifest["links_dedup_refs"],
        "retrieval_units": manifest["retrieval_units"],
        "table_candidates": manifest["table_candidates"],
        "manifest_path": manifest["manifest_path"],
    }


def test_redacted_fixture_manifest_is_metadata_only_json_contract() -> None:
    manifest = _load_fixture("minimal_manifest.json")

    assert manifest["schema_version"] == "m024-article-retrieval-tables.v1"
    assert manifest["manifest_schema"] == manifest["schema_version"]
    assert manifest["summary"]["import_eligible_count"] == 0
    assert manifest["summary"]["promoted_to_fact_count"] == 0
    assert manifest["summary"]["ladybugdb_written_count"] == 0
    assert manifest["summary"]["production_import_attempted_count"] == 0
    assert manifest["summary"]["graph_readiness_count"] == 0
    assert set(manifest["summary"]["diagnostic_counts"]) == EXPECTED_DIAGNOSTIC_COUNTER_KEYS
    assert manifest["bridge_subtree"] == {
        **manifest["bridge_subtree"],
        "graph_import_claim": False,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "raw_payloads_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "optimizer_traces_included": False,
    }
    assert manifest["safety_flags"] == {
        **manifest["safety_flags"],
        "metadata_only": True,
        "cpu_only": True,
        "review_only": True,
        "raw_payloads_included": False,
        "table_text_included": False,
        "caption_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "dspy_used": False,
        "rlm_used": False,
        "optimizer_used": False,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }
    _assert_metadata_only(manifest)


def test_minimal_manifest_validates_schema_counters_and_review_only_statuses(
    retrieval_tables_contract,
) -> None:
    manifest = _load_fixture("minimal_manifest.json")

    assert (
        retrieval_tables_contract.ARTICLE_RETRIEVAL_TABLES_SCHEMA_VERSION
        == "m024-article-retrieval-tables.v1"
    )
    assert retrieval_tables_contract.validate_article_retrieval_table_manifest(manifest) == []
    assert set(retrieval_tables_contract.ALLOWED_BENCHMARK_STATUSES) == EXPECTED_VALID_STATUSES
    assert (
        set(retrieval_tables_contract.DIAGNOSTIC_COUNTER_KEYS) == EXPECTED_DIAGNOSTIC_COUNTER_KEYS
    )
    assert manifest["summary"] == {
        "retrieval_unit_count": 3,
        "table_candidate_count": 1,
        "included_review_only_count": 4,
        "blocked_count": 0,
        "repair_required_count": 0,
        "source_ref_count": 2,
        "page_index_node_ref_count": 3,
        "page_index_anchor_ref_count": 3,
        "asset_ref_count": 1,
        "link_provenance_ref_count": 3,
        "manifest_provenance_count": 3,
        "ranking_tie_count": 1,
        "diagnostic_counts": {
            "duplicate_id_count": 0,
            "malformed_source_ref_count": 0,
            "missing_page_index_provenance_count": 0,
            "missing_asset_provenance_count": 0,
            "bad_vocabulary_count": 0,
            "forbidden_payload_detection_count": 0,
            "unsafe_authorization_count": 0,
            "unsafe_readiness_count": 0,
        },
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "ladybugdb_written_count": 0,
        "production_import_attempted_count": 0,
        "graph_readiness_count": 0,
    }
    assert {unit["benchmark_status"] for unit in manifest["retrieval_units"]} == {
        "included_review_only"
    }
    assert {candidate["benchmark_status"] for candidate in manifest["table_candidates"]} == {
        "included_review_only"
    }
    _assert_metadata_only(manifest)


def test_build_manifest_ranks_retrieval_units_with_stable_cpu_only_tiebreakers(
    retrieval_tables_contract,
) -> None:
    fixture = _load_fixture("minimal_manifest.json")
    built = retrieval_tables_contract.build_article_retrieval_table_manifest(
        **_builder_kwargs(fixture)
    )

    assert retrieval_tables_contract.validate_article_retrieval_table_manifest(built) == []
    assert [unit["unit_id"] for unit in built["retrieval_units"]] == [
        "fixture-paper-0001:retrieval-unit:section:methods",
        "fixture-paper-0001:retrieval-unit:section:results",
        "fixture-paper-0001:retrieval-unit:table:0001",
    ]
    assert [unit["rank"] for unit in built["retrieval_units"]] == [1, 2, 3]
    assert (
        built["retrieval_units"][0]["benchmark_score"]
        == built["retrieval_units"][1]["benchmark_score"]
    )
    assert built["retrieval_units"][0]["unit_id"] < built["retrieval_units"][1]["unit_id"]
    assert built["summary"]["ranking_tie_count"] == 1
    assert built["safety_flags"]["cpu_only"] is True
    assert built["safety_flags"]["dspy_used"] is False
    assert built["safety_flags"]["rlm_used"] is False
    assert built["safety_flags"]["optimizer_used"] is False
    _assert_metadata_only(built)


def test_table_candidates_preserve_asset_page_index_and_source_provenance_without_table_payloads(
    retrieval_tables_contract,
) -> None:
    manifest = _load_fixture("minimal_manifest.json")
    candidate = manifest["table_candidates"][0]

    assert retrieval_tables_contract.validate_article_retrieval_table_manifest(manifest) == []
    assert candidate == {
        **candidate,
        "candidate_family": "asset_page_index_table_candidate",
        "asset_id": "fixture-paper-0001:asset:table:0001",
        "page_index_node_id": "fixture-paper-0001:page-index:artifact:table:0001",
        "page_index_anchor_id": "fixture-paper-0001:page-index-anchor:table-0001",
        "source_ref_ids": ["fixture-paper-0001:source:pdf"],
        "source_span_ids": ["fixture-paper-0001:span:table-0001"],
        "raw_table_embedded": False,
        "caption_embedded": False,
        "embedding_included": False,
        "vector_included": False,
        "import_eligible": False,
        "promoted_to_fact": False,
    }
    assert candidate["transformation_plan"] == {
        "status": "review_only_not_executed",
        "operation": "metadata_only_table_candidate",
        "raw_table_cells_included": False,
        "caption_included": False,
    }
    _assert_metadata_only(candidate)


def test_summary_helper_returns_aggregate_benchmark_and_fixed_zero_import_counters(
    retrieval_tables_contract,
) -> None:
    manifest = _load_fixture("minimal_manifest.json")

    summary = retrieval_tables_contract.summarize_article_retrieval_tables(manifest)

    assert summary["retrieval_unit_count"] == 3
    assert summary["table_candidate_count"] == 1
    assert summary["included_review_only_count"] == 4
    assert summary["source_ref_count"] == 2
    assert summary["page_index_node_ref_count"] == 3
    assert summary["page_index_anchor_ref_count"] == 3
    assert summary["asset_ref_count"] == 1
    assert summary["link_provenance_ref_count"] == 3
    assert summary["manifest_provenance_count"] == 3
    assert summary["diagnostic_counts"] == dict.fromkeys(EXPECTED_DIAGNOSTIC_COUNTER_KEYS, 0)
    assert summary["import_eligible_count"] == 0
    assert summary["promoted_to_fact_count"] == 0
    assert summary["ladybugdb_written_count"] == 0
    assert summary["production_import_attempted_count"] == 0
    assert summary["graph_readiness_count"] == 0


def test_to_redacted_dict_and_to_json_never_emit_forbidden_payloads_or_unsafe_flags(
    retrieval_tables_contract,
) -> None:
    manifest = _load_fixture("minimal_manifest.json")

    redacted = retrieval_tables_contract.to_redacted_dict(manifest)
    serialized = retrieval_tables_contract.to_json(manifest)
    decoded = json.loads(serialized)

    assert decoded == redacted
    assert serialized == json.dumps(redacted, sort_keys=True, indent=2) + "\n"
    _assert_metadata_only(redacted)
    _assert_metadata_only(decoded)


@pytest.mark.parametrize(
    ("mutator", "expected_code", "expected_path"),
    [
        (
            lambda manifest: manifest["retrieval_units"].append(
                deepcopy(manifest["retrieval_units"][0])
            ),
            "duplicate_id",
            "$.retrieval_units[3].unit_id",
        ),
        (
            lambda manifest: manifest["source_refs"][0].update(
                {"source_id": "bad source id with spaces"}
            ),
            "malformed_source_ref",
            "$.source_refs[0].source_id",
        ),
        (
            lambda manifest: manifest["retrieval_units"][0].update(
                {"page_index_anchor_id": "fixture-paper-0001:page-index-anchor:missing"}
            ),
            "missing_page_index_provenance",
            "$.retrieval_units[0].page_index_anchor_id",
        ),
        (
            lambda manifest: manifest["table_candidates"][0].update(
                {"asset_id": "fixture-paper-0001:asset:table:missing"}
            ),
            "missing_asset_provenance",
            "$.table_candidates[0].asset_id",
        ),
        (
            lambda manifest: manifest["table_candidates"][0].update(
                {"benchmark_status": "accepted_for_import"}
            ),
            "bad_vocabulary",
            "$.table_candidates[0].benchmark_status",
        ),
        (
            lambda manifest: manifest["table_candidates"][0].update(
                {"caption_text": "FORBIDDEN_CAPTION_TEXT_DO_NOT_ECHO"}
            ),
            "forbidden_payload_key",
            "$.table_candidates[0].caption_text",
        ),
        (
            lambda manifest: manifest["retrieval_units"][0].update({"embedding": [0.1, 0.2, 0.3]}),
            "forbidden_payload_key",
            "$.retrieval_units[0].embedding",
        ),
        (
            lambda manifest: manifest["retrieval_units"][0].update({"import_eligible": True}),
            "unsafe_authorization",
            "$.retrieval_units[0].import_eligible",
        ),
        (
            lambda manifest: manifest["bridge_subtree"].update({"trusted_kg_import_allowed": True}),
            "unsafe_authorization",
            "$.bridge_subtree.trusted_kg_import_allowed",
        ),
        (
            lambda manifest: manifest["bridge_subtree"].update({"status": "ready_for_import"}),
            "unsafe_readiness",
            "$.bridge_subtree.status",
        ),
    ],
)
def test_validate_manifest_reports_negative_cases_with_stable_codes_and_paths(
    retrieval_tables_contract,
    mutator,
    expected_code: str,
    expected_path: str,
) -> None:
    manifest = _load_fixture("minimal_manifest.json")
    mutator(manifest)

    diagnostics = retrieval_tables_contract.validate_article_retrieval_table_manifest(manifest)
    diagnostic_manifest = {
        "diagnostics": [diagnostic.to_redacted_dict() for diagnostic in diagnostics]
    }

    assert expected_code in _diagnostic_codes(diagnostic_manifest)
    assert expected_path in _diagnostic_paths(diagnostic_manifest)
    _assert_metadata_only(diagnostic_manifest)


def test_unsafe_fixture_collects_all_safety_and_provenance_failures(
    retrieval_tables_contract,
) -> None:
    manifest = _load_fixture("unsafe_manifest.json")

    diagnostics = retrieval_tables_contract.validate_article_retrieval_table_manifest(manifest)
    diagnostic_manifest = {
        "diagnostics": [diagnostic.to_redacted_dict() for diagnostic in diagnostics]
    }
    codes = _diagnostic_codes(diagnostic_manifest)

    assert {
        "duplicate_id",
        "malformed_source_ref",
        "missing_page_index_provenance",
        "missing_asset_provenance",
        "bad_vocabulary",
        "forbidden_payload_key",
        "unsafe_authorization",
        "unsafe_readiness",
    } <= codes
    assert "$.retrieval_units[0].raw_text" in _diagnostic_paths(diagnostic_manifest)
    assert "$.retrieval_units[0].embedding" in _diagnostic_paths(diagnostic_manifest)
    assert "$.retrieval_units[0].vector" in _diagnostic_paths(diagnostic_manifest)
    assert "$.table_candidates[0].table_text" in _diagnostic_paths(diagnostic_manifest)
    assert "$.table_candidates[0].caption_text" in _diagnostic_paths(diagnostic_manifest)
    _assert_metadata_only(diagnostic_manifest)


def test_validation_does_not_read_gitignored_planning_or_audit_artifacts(
    retrieval_tables_contract, monkeypatch
) -> None:
    manifest = _load_fixture("minimal_manifest.json")
    original_read_text = Path.read_text

    def guarded_read_text(self: Path, *args: Any, **kwargs: Any) -> str:
        normalized = self.as_posix()
        forbidden_parts = ("/.gsd/", "/.planning/", "/.audits/", ".gsd/", ".planning/", ".audits/")
        if any(part in normalized for part in forbidden_parts):
            raise AssertionError(f"validator must not read gitignored artifact path: {normalized}")
        return original_read_text(self, *args, **kwargs)

    monkeypatch.setattr(Path, "read_text", guarded_read_text)

    assert retrieval_tables_contract.validate_article_retrieval_table_manifest(manifest) == []
