"""Contract tests for metadata-only article link and dedup evidence bundles.

S05 is intentionally test-first.  The future ``arxiv_archive.article_links_dedup``
module must expose deterministic, review-only link/dedup manifests that separate
citation links, structural links, metadata signals, and preprint dedup candidates.
The contract is metadata-only: diagnostics may identify unsafe paths, but redacted
manifests must never carry article prose, raw references, abstracts, model output,
binary payloads, vectors, secrets, graph-write claims, or import authorization.
"""

from __future__ import annotations

import importlib
import json
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_links_dedup"

FORBIDDEN_SENTINELS = (
    "FORBIDDEN_RAW_TITLE_DO_NOT_ECHO",
    "FORBIDDEN_RAW_REFERENCE_DO_NOT_ECHO",
    "FORBIDDEN_RAW_ABSTRACT_DO_NOT_ECHO",
    "secret-token",
    "session=abc",
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
    "title",
    "abstract",
    "reference",
    "references",
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
    '"model_outputs_included": true',
    '"raw_payloads_included": true',
    '"import_eligible": true',
    '"promoted_to_fact": true',
)
EXPECTED_DIAGNOSTIC_COUNTER_KEYS = {
    "duplicate_id_count",
    "malformed_source_ref_count",
    "missing_page_index_anchor_count",
    "bad_vocabulary_count",
    "conflict_count",
    "insufficient_metadata_count",
    "forbidden_payload_detection_count",
    "unsafe_authorization_count",
}


@pytest.fixture()
def links_dedup_contract():
    """Load the future implementation without making collection fail.

    T01 defines the red/green target.  Until T02 adds the module, pytest reports
    these tests as expected failures rather than import-time collection errors.
    """

    try:
        return importlib.import_module("arxiv_archive.article_links_dedup")
    except ModuleNotFoundError as exc:
        if exc.name == "arxiv_archive.article_links_dedup":
            pytest.xfail("arxiv_archive.article_links_dedup is not implemented yet")
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


def test_redacted_fixture_manifests_are_metadata_only_json_contracts() -> None:
    minimal = _load_fixture("minimal_manifest.json")
    conflict = _load_fixture("conflict_manifest.json")

    for manifest in (minimal, conflict):
        assert manifest["schema_version"] == "m024-article-links-dedup.v1"
        assert manifest["summary"]["import_eligible_count"] == 0
        assert manifest["import_eligible_count"] == 0
        assert manifest["promoted_to_fact_count"] == 0
        assert set(manifest["summary"]["diagnostic_counts"]) == EXPECTED_DIAGNOSTIC_COUNTER_KEYS
        assert manifest["bridge_subtree"] == {
            **manifest["bridge_subtree"],
            "graph_import_claim": False,
            "trusted_kg_import_allowed": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
            "raw_payloads_included": False,
        }
        _assert_metadata_only(manifest)


def test_minimal_manifest_validates_family_schema_and_benchmark_counters(links_dedup_contract) -> None:
    manifest = _load_fixture("minimal_manifest.json")

    assert links_dedup_contract.ARTICLE_LINKS_DEDUP_SCHEMA_VERSION == "m024-article-links-dedup.v1"
    assert links_dedup_contract.validate_article_links_dedup_manifest(manifest) == []
    assert manifest["summary"]["link_family_counts"] == {
        "citation": 1,
        "structural": 1,
        "metadata_signal": 4,
        "dedup_candidate": 1,
    }
    assert manifest["summary"]["metadata_signal_counts"] == {
        "doi": 1,
        "arxiv_id": 1,
        "url": 1,
        "content_hash": 1,
    }
    assert manifest["summary"]["dedup_decision_counts"] == {
        "candidate_same_work_review_required": 1,
    }
    assert manifest["summary"]["page_index_anchor_coverage"] == {
        "required_anchor_ref_count": 7,
        "covered_anchor_ref_count": 7,
        "missing_anchor_ref_count": 0,
    }
    assert manifest["summary"]["source_span_coverage"] == {
        "required_source_span_ref_count": 7,
        "covered_source_span_ref_count": 7,
        "missing_source_span_ref_count": 0,
    }
    assert manifest["summary"]["diagnostic_counts"] == {
        "duplicate_id_count": 0,
        "malformed_source_ref_count": 0,
        "missing_page_index_anchor_count": 0,
        "bad_vocabulary_count": 0,
        "conflict_count": 0,
        "insufficient_metadata_count": 0,
        "forbidden_payload_detection_count": 0,
        "unsafe_authorization_count": 0,
    }
    assert [link["link_id"] for link in manifest["citation_links"]] == [
        "fixture-paper-0001:link:citation:0001"
    ]
    assert [link["link_id"] for link in manifest["structural_links"]] == [
        "fixture-paper-0001:link:structural:0001"
    ]
    assert [signal["signal_id"] for signal in manifest["metadata_signals"]] == [
        "fixture-paper-0001:metadata-signal:doi:0001",
        "fixture-paper-0001:metadata-signal:arxiv:0001",
        "fixture-paper-0001:metadata-signal:url:0001",
        "fixture-paper-0001:metadata-signal:hash:0001",
    ]
    assert [candidate["candidate_id"] for candidate in manifest["dedup_candidates"]] == [
        "fixture-paper-0001:dedup:preprint:0001"
    ]
    _assert_metadata_only(manifest)


def test_normalization_helpers_canonicalize_metadata_signals_without_secret_url_tokens(links_dedup_contract) -> None:
    assert links_dedup_contract.normalize_doi(" https://doi.org/10.48550/ARXIV.2605.00001 ") == "10.48550/arxiv.2605.00001"
    assert links_dedup_contract.normalize_arxiv_id(" arXiv:2605.00001V2 ") == "2605.00001v2"
    assert links_dedup_contract.normalize_hash_signal("SHA256:CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC") == (
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
    )
    assert links_dedup_contract.normalize_url(
        "HTTPS://Example.ORG/papers/2605.00001?token=secret-token&session=abc#abstract"
    ) == "https://example.org/papers/2605.00001"


def test_empty_family_lists_are_valid_but_report_zero_coverage_shape(links_dedup_contract) -> None:
    manifest = deepcopy(_load_fixture("minimal_manifest.json"))
    manifest["citation_links"] = []
    manifest["structural_links"] = []
    manifest["metadata_signals"] = []
    manifest["dedup_candidates"] = []

    rebuilt = links_dedup_contract.build_article_links_dedup_manifest(manifest)

    assert links_dedup_contract.validate_article_links_dedup_manifest(rebuilt) == []
    assert rebuilt["summary"]["link_family_counts"] == {
        "citation": 0,
        "structural": 0,
        "metadata_signal": 0,
        "dedup_candidate": 0,
    }
    assert rebuilt["summary"]["metadata_signal_counts"] == {}
    assert rebuilt["summary"]["dedup_decision_counts"] == {}
    assert rebuilt["summary"]["diagnostic_counts"]["insufficient_metadata_count"] == 0
    assert rebuilt["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(rebuilt)


def test_malformed_refs_duplicate_ids_bad_vocabularies_and_unsafe_flags_fail_closed(links_dedup_contract) -> None:
    manifest = deepcopy(_load_fixture("minimal_manifest.json"))
    manifest["source_refs"][0]["sha256"] = "not-a-sha256"
    manifest["citation_links"].append(deepcopy(manifest["citation_links"][0]))
    manifest["citation_links"][0]["source_page_index_anchor_id"] = "fixture-paper-0001:page-index-anchor:missing"
    manifest["structural_links"][0]["relationship"] = "contains_raw_payload"
    manifest["structural_links"][0]["review_state"] = "trusted"
    manifest["dedup_candidates"][0]["decision"] = "auto_merge"
    manifest["dedup_candidates"][0]["import_eligible"] = True
    manifest["safety_flags"]["trusted_kg_import_allowed"] = True

    rebuilt = links_dedup_contract.build_article_links_dedup_manifest(manifest)
    diagnostics = links_dedup_contract.validate_article_links_dedup_manifest(rebuilt)
    combined = {"diagnostics": rebuilt["diagnostics"] + diagnostics}
    codes = _diagnostic_codes(combined)
    paths = _diagnostic_paths(combined)

    assert {
        "malformed_source_ref",
        "duplicate_id",
        "missing_page_index_anchor",
        "unsupported_structural_relationship",
        "unsupported_review_state",
        "unsupported_dedup_decision",
        "unsafe_import_flag_true:trusted_kg_import_allowed",
        "unsafe_import_flag_true:import_eligible",
    } <= codes
    assert "/source_refs[0]/sha256" in paths
    assert "/citation_links[1]/link_id" in paths
    assert "/citation_links[0]/source_page_index_anchor_id" in paths
    assert "/structural_links[0]/relationship" in paths
    assert "/dedup_candidates[0]/decision" in paths
    assert rebuilt["summary"]["diagnostic_counts"]["duplicate_id_count"] == 1
    assert rebuilt["summary"]["diagnostic_counts"]["malformed_source_ref_count"] == 1
    assert rebuilt["summary"]["diagnostic_counts"]["missing_page_index_anchor_count"] == 1
    assert rebuilt["summary"]["diagnostic_counts"]["bad_vocabulary_count"] >= 3
    assert rebuilt["summary"]["diagnostic_counts"]["unsafe_authorization_count"] == 2
    assert rebuilt["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(rebuilt)


def test_raw_payload_keys_are_diagnosed_and_redacted_from_output_manifest(links_dedup_contract) -> None:
    manifest = deepcopy(_load_fixture("minimal_manifest.json"))
    manifest["source_refs"][0]["title"] = "FORBIDDEN_RAW_TITLE_DO_NOT_ECHO"
    manifest["citation_links"][0]["target_ref"]["reference"] = "FORBIDDEN_RAW_REFERENCE_DO_NOT_ECHO"
    manifest["metadata_signals"][0]["abstract"] = "FORBIDDEN_RAW_ABSTRACT_DO_NOT_ECHO"

    rebuilt = links_dedup_contract.build_article_links_dedup_manifest(manifest)
    diagnostics = links_dedup_contract.validate_article_links_dedup_manifest(rebuilt)
    combined = {"diagnostics": rebuilt["diagnostics"] + diagnostics}

    assert _diagnostic_codes(combined) >= {"forbidden_payload_key"}
    assert {
        "/source_refs[0]/title",
        "/citation_links[0]/target_ref/reference",
        "/metadata_signals[0]/abstract",
    } <= _diagnostic_paths(combined)
    assert rebuilt["summary"]["diagnostic_counts"]["forbidden_payload_detection_count"] == 3
    assert rebuilt["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(rebuilt)


def test_url_query_tokens_conflicting_signals_and_insufficient_metadata_are_reported(links_dedup_contract) -> None:
    manifest = deepcopy(_load_fixture("minimal_manifest.json"))
    manifest["metadata_signals"][0]["normalized_value"] = "10.48550/arxiv.2605.00002"
    manifest["metadata_signals"].append(
        {
            "signal_id": "fixture-paper-0001:metadata-signal:doi:0002",
            "signal_type": "doi",
            "normalized_value": "10.9999/conflicting-doi",
            "source_page_index_anchor_id": "fixture-paper-0001:page-index-anchor:citation-0001",
            "source_span_id": "fixture-paper-0001:span:citation-0001",
            "review_state": "review_required",
        }
    )
    manifest["metadata_signals"].append(
        {
            "signal_id": "fixture-paper-0001:metadata-signal:url:0002",
            "signal_type": "url",
            "normalized_value": "https://example.org/paper?token=secret-token&session=abc",
            "source_page_index_anchor_id": "fixture-paper-0001:page-index-anchor:citation-0001",
            "source_span_id": "fixture-paper-0001:span:citation-0001",
            "review_state": "review_required",
        }
    )
    manifest["dedup_candidates"] = [
        {
            "candidate_id": "fixture-paper-0001:dedup:preprint:0001",
            "candidate_family": "preprint_dedup",
            "decision": "conflicting_metadata_review_required",
            "source_record_ref": "fixture-paper-0001:reference:0001",
            "target_record_ref": "arxiv:2605.00002",
            "evidence_signal_ids": [
                "fixture-paper-0001:metadata-signal:doi:0001",
                "fixture-paper-0001:metadata-signal:doi:0002",
            ],
            "confidence_label": "low",
            "review_state": "review_required",
            "import_eligible": False,
        },
        {
            "candidate_id": "fixture-paper-0001:dedup:preprint:0002",
            "candidate_family": "preprint_dedup",
            "decision": "insufficient_metadata_review_required",
            "source_record_ref": "fixture-paper-0001:reference:0002",
            "target_record_ref": None,
            "evidence_signal_ids": [],
            "confidence_label": "low",
            "review_state": "review_required",
            "import_eligible": False,
        },
    ]

    rebuilt = links_dedup_contract.build_article_links_dedup_manifest(manifest)
    diagnostics = links_dedup_contract.validate_article_links_dedup_manifest(rebuilt)
    combined = {"diagnostics": rebuilt["diagnostics"] + diagnostics}

    assert {
        "url_query_tokens_removed",
        "conflicting_metadata_signals",
        "insufficient_metadata_for_dedup",
    } <= _diagnostic_codes(combined)
    assert rebuilt["summary"]["diagnostic_counts"]["conflict_count"] == 1
    assert rebuilt["summary"]["diagnostic_counts"]["insufficient_metadata_count"] == 1
    assert rebuilt["summary"]["dedup_decision_counts"] == {
        "conflicting_metadata_review_required": 1,
        "insufficient_metadata_review_required": 1,
    }
    assert rebuilt["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(rebuilt)


def test_conflict_fixture_documents_redacted_diagnostics_for_later_aggregation(links_dedup_contract) -> None:
    manifest = _load_fixture("conflict_manifest.json")
    diagnostics = links_dedup_contract.validate_article_links_dedup_manifest(manifest)
    combined = {"diagnostics": manifest["diagnostics"] + diagnostics}

    assert {
        "missing_page_index_anchor",
        "missing_source_span",
        "conflicting_metadata_signals",
        "insufficient_metadata_for_dedup",
        "forbidden_payload_key",
        "unsafe_import_flag_true:trusted_kg_import_allowed",
        "unsafe_import_flag_true:import_eligible",
    } <= _diagnostic_codes(combined)
    assert manifest["summary"]["diagnostic_counts"] == {
        "duplicate_id_count": 0,
        "malformed_source_ref_count": 0,
        "missing_page_index_anchor_count": 1,
        "bad_vocabulary_count": 0,
        "conflict_count": 1,
        "insufficient_metadata_count": 1,
        "forbidden_payload_detection_count": 3,
        "unsafe_authorization_count": 2,
    }
    assert manifest["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(manifest)
