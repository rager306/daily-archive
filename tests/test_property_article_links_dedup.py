"""Property hardening for metadata-only article link/dedup manifests."""

from __future__ import annotations

import json
from copy import deepcopy
from typing import Any

from hypothesis import given, settings
from hypothesis import strategies as st

from research_graph.infrastructure.papers.indexing.links_dedup import (
    ALLOWED_DEDUP_DECISIONS,
    ALLOWED_METADATA_SIGNAL_TYPES,
    ALLOWED_STRUCTURAL_RELATIONSHIPS,
    build_article_links_dedup_manifest,
    deterministic_id,
    normalize_arxiv_id,
    normalize_doi,
    normalize_hash_signal,
    normalize_url,
    summarize_article_links_dedup,
    to_json,
    validate_article_links_dedup_manifest,
)

HEX64 = st.text(alphabet="0123456789abcdef", min_size=64, max_size=64)
SAFE_SLUG = st.from_regex(r"[a-z][a-z0-9]{2,12}", fullmatch=True)
SAFE_VALUE = st.text(
    alphabet=st.characters(blacklist_categories=("Cs",), blacklist_characters="\x00"),
    min_size=1,
    max_size=50,
).filter(lambda value: value.strip() != "")
PAPER_IDS = st.from_regex(r"property-paper-[a-f0-9]{4,10}", fullmatch=True)
DOI_SUFFIX = st.from_regex(r"10\.\d{4,5}/[A-Za-z0-9._;()/:+-]{3,24}", fullmatch=True)
ARXIV_IDS = st.from_regex(r"\d{4}\.\d{4,5}(v\d{1,2})?", fullmatch=True)
URL_HOSTS = st.from_regex(r"[a-z0-9]{3,12}\.example\.org", fullmatch=True)
FORBIDDEN_PAYLOAD_KEYS = st.sampled_from(
    [
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "title",
        "abstract",
        "reference",
        "references",
        "model_output",
        "payload",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "secret",
        "token",
        "api_key",
        "credentials",
        "optimizer_trace",
    ]
)
UNSAFE_FLAG_KEYS = st.sampled_from(
    [
        "trusted_kg_import_allowed",
        "ladybugdb_written",
        "production_import_attempted",
        "model_outputs_included",
        "raw_payloads_included",
        "import_eligible",
        "promoted_to_fact",
    ]
)
FORBIDDEN_SENTINELS = (
    "FORBIDDEN_RAW_TITLE_DO_NOT_ECHO",
    "FORBIDDEN_RAW_REFERENCE_DO_NOT_ECHO",
    "FORBIDDEN_RAW_ABSTRACT_DO_NOT_ECHO",
    "sk-test-secret",
    "embedding=[0.1, 0.2]",
    "vector=[0.3, 0.4]",
    "token=secret-token",
    "session=abc",
)


def _manifest(paper_id: str, *, metadata_signal_count: int = 2) -> dict[str, Any]:
    anchor_id = f"{paper_id}:page-index-anchor:citation-0001"
    span_id = f"{paper_id}:span:citation-0001"
    signal_templates = [
        ("doi", "10.48550/arxiv.2605.00001"),
        ("arxiv_id", "2605.00001v1"),
        ("url", "https://example.org/papers/2605.00001"),
        ("content_hash", "c" * 64),
    ]
    metadata_signals = [
        {
            "signal_id": f"{paper_id}:metadata-signal:{signal_type}:{index:04d}",
            "signal_type": signal_type,
            "normalized_value": normalized_value,
            "source_page_index_anchor_id": anchor_id,
            "source_span_id": span_id,
            "review_state": "review_required",
        }
        for index, (signal_type, normalized_value) in enumerate(
            signal_templates[:metadata_signal_count], start=1
        )
    ]
    return {
        "paper_id": paper_id,
        "run_id": f"m024-property-{paper_id}",
        "source_refs": [
            {
                "source_id": f"{paper_id}:source:normalized-md",
                "paper_id": paper_id,
                "source_role": "normalized_markdown",
                "source_path": f"fixtures/{paper_id}/normalized.md",
                "sha256": "a" * 64,
                "media_type": "text/markdown",
            }
        ],
        "page_index_refs": {
            "schema_version": "m024-page-index.v1",
            "manifest_path": "artifacts/page-index-manifest.json",
            "manifest_sha256": "b" * 64,
            "node_ids": [f"{paper_id}:page-index:section:results"],
            "anchor_ids": [anchor_id],
        },
        "citation_links": [
            {
                "link_id": f"{paper_id}:link:citation:0001",
                "source_page_index_node_id": f"{paper_id}:page-index:section:results",
                "source_page_index_anchor_id": anchor_id,
                "target_ref": {
                    "target_type": "reference_entry",
                    "target_id": f"{paper_id}:reference:0001",
                },
                "source_span_ids": [span_id],
                "evidence_signal_ids": [metadata_signals[0]["signal_id"]]
                if metadata_signals
                else [],
                "review_state": "review_required",
            }
        ],
        "structural_links": [
            {
                "link_id": f"{paper_id}:link:structural:0001",
                "relationship": "located_in",
                "source_page_index_node_id": f"{paper_id}:page-index:artifact:reference:0001",
                "target_page_index_node_id": f"{paper_id}:page-index:section:results",
                "source_page_index_anchor_id": anchor_id,
                "source_span_ids": [span_id],
                "review_state": "review_required",
            }
        ],
        "metadata_signals": metadata_signals,
        "dedup_candidates": [
            {
                "candidate_id": f"{paper_id}:dedup:preprint:0001",
                "candidate_family": "preprint_dedup",
                "decision": "candidate_same_work_review_required",
                "source_record_ref": f"{paper_id}:reference:0001",
                "target_record_ref": "arxiv:2605.00001v1",
                "evidence_signal_ids": [signal["signal_id"] for signal in metadata_signals],
                "confidence_label": "medium",
                "review_state": "review_required",
                "import_eligible": False,
            }
        ],
    }


def _assert_metadata_only(payload: dict[str, Any]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    for sentinel in FORBIDDEN_SENTINELS:
        assert sentinel not in serialized
    assert '"trusted_kg_import_allowed": true' not in serialized
    assert '"ladybugdb_written": true' not in serialized
    assert '"production_import_attempted": true' not in serialized
    assert '"import_eligible": true' not in serialized
    assert '"promoted_to_fact": true' not in serialized


def _diagnostic_codes(payload: dict[str, Any]) -> set[str]:
    return {str(diagnostic["code"]) for diagnostic in payload.get("diagnostics", [])}


@settings(max_examples=80)
@given(
    prefix=SAFE_SLUG,
    parts=st.lists(
        st.one_of(SAFE_VALUE, st.integers(), st.booleans(), st.none()), min_size=0, max_size=5
    ),
)
def test_deterministic_id_is_stable_prefix_scoped_and_json_safe(
    prefix: str, parts: list[Any]
) -> None:
    first = deterministic_id(prefix, *parts)
    second = deterministic_id(prefix, *deepcopy(parts))
    alternate_prefix = deterministic_id(f"other-{prefix}", *parts)

    assert first == second
    assert first.startswith(f"{prefix}:")
    assert alternate_prefix.startswith(f"other-{prefix}:")
    assert alternate_prefix != first
    assert json.loads(json.dumps({"id": first}, sort_keys=True)) == {"id": first}


@settings(max_examples=80)
@given(doi=DOI_SUFFIX)
def test_doi_normalization_strips_resolver_prefixes_case_and_padding(doi: str) -> None:
    expected = doi.lower()

    assert normalize_doi(f" https://doi.org/{doi.upper()} ") == expected
    assert normalize_doi(f"doi: {doi.upper()}") == expected
    assert normalize_doi(normalize_doi(f" https://dx.doi.org/{doi.upper()} ")) == expected


@settings(max_examples=80)
@given(arxiv_id=ARXIV_IDS)
def test_arxiv_normalization_strips_known_prefixes_without_dropping_version(arxiv_id: str) -> None:
    expected = arxiv_id.lower()

    assert normalize_arxiv_id(f" arXiv: {arxiv_id.upper()} ") == expected
    assert normalize_arxiv_id(f"https://arxiv.org/abs/{arxiv_id.upper()}") == expected
    assert normalize_arxiv_id(f"https://arxiv.org/pdf/{arxiv_id.upper()}.pdf") == expected


@settings(max_examples=80)
@given(host=URL_HOSTS, path=SAFE_SLUG, token=SAFE_SLUG)
def test_url_normalization_removes_query_fragments_and_is_idempotent(
    host: str, path: str, token: str
) -> None:
    url = f"HTTPS://{host.upper()}/papers/{path}/?token={token}&session=abc#abstract"
    normalized = normalize_url(url)

    assert normalized == f"https://{host}/papers/{path}"
    assert normalize_url(normalized) == normalized
    assert "?" not in normalized
    assert "#" not in normalized
    assert "token" not in normalized


@settings(max_examples=60)
@given(hash_value=HEX64)
def test_hash_signal_normalization_is_lowercase_idempotent(hash_value: str) -> None:
    normalized = normalize_hash_signal(f"SHA256:{hash_value.upper()}")

    assert normalized == hash_value
    assert normalize_hash_signal(normalized) == normalized


@settings(max_examples=60)
@given(paper_id=PAPER_IDS)
def test_manifest_build_and_json_serialization_are_deterministic_metadata_only(
    paper_id: str,
) -> None:
    manifest = _manifest(paper_id, metadata_signal_count=3)

    first = build_article_links_dedup_manifest(deepcopy(manifest))
    second = build_article_links_dedup_manifest(deepcopy(manifest))

    assert first == second
    assert json.loads(to_json(first)) == first
    assert to_json(first) == to_json(second)
    assert validate_article_links_dedup_manifest(first) == []
    assert first["summary"] == summarize_article_links_dedup(first)
    assert first["summary"]["import_eligible_count"] == 0
    assert first["import_eligible_count"] == 0
    assert first["promoted_to_fact_count"] == 0
    _assert_metadata_only(first)


@settings(max_examples=80)
@given(
    paper_id=PAPER_IDS,
    relationship=st.text(min_size=1, max_size=24).filter(
        lambda value: value not in ALLOWED_STRUCTURAL_RELATIONSHIPS
    ),
    signal_type=st.text(min_size=1, max_size=24).filter(
        lambda value: value not in ALLOWED_METADATA_SIGNAL_TYPES
    ),
    decision=st.text(min_size=1, max_size=40).filter(
        lambda value: value not in ALLOWED_DEDUP_DECISIONS
    ),
)
def test_unsupported_vocabularies_emit_bad_vocabulary_diagnostics_not_crashes(
    paper_id: str, relationship: str, signal_type: str, decision: str
) -> None:
    manifest = _manifest(paper_id)
    manifest["structural_links"][0]["relationship"] = relationship
    manifest["metadata_signals"][0]["signal_type"] = signal_type
    manifest["dedup_candidates"][0]["decision"] = decision

    rebuilt = build_article_links_dedup_manifest(manifest)

    assert validate_article_links_dedup_manifest(rebuilt)
    assert rebuilt["summary"]["diagnostic_counts"]["bad_vocabulary_count"] >= 3
    assert rebuilt["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(rebuilt)


@settings(max_examples=80)
@given(paper_id=PAPER_IDS)
def test_duplicate_ids_conflicting_signals_and_insufficient_candidates_are_counted_fail_closed(
    paper_id: str,
) -> None:
    manifest = _manifest(paper_id, metadata_signal_count=2)
    manifest["citation_links"].append(deepcopy(manifest["citation_links"][0]))
    manifest["metadata_signals"][1]["signal_type"] = "doi"
    manifest["metadata_signals"][1]["normalized_value"] = "10.9999/conflict"
    manifest["dedup_candidates"] = [
        {
            **manifest["dedup_candidates"][0],
            "decision": "conflicting_metadata_review_required",
            "evidence_signal_ids": [
                manifest["metadata_signals"][0]["signal_id"],
                manifest["metadata_signals"][1]["signal_id"],
            ],
        },
        {
            **manifest["dedup_candidates"][0],
            "candidate_id": f"{paper_id}:dedup:preprint:0002",
            "decision": "insufficient_metadata_review_required",
            "target_record_ref": None,
            "evidence_signal_ids": [],
        },
    ]

    rebuilt = build_article_links_dedup_manifest(manifest)
    codes = _diagnostic_codes(rebuilt)

    assert {
        "duplicate_id",
        "conflicting_metadata_signals",
        "insufficient_metadata_for_dedup",
    } <= codes
    assert rebuilt["summary"]["diagnostic_counts"]["duplicate_id_count"] == 1
    assert rebuilt["summary"]["diagnostic_counts"]["conflict_count"] == 1
    assert rebuilt["summary"]["diagnostic_counts"]["insufficient_metadata_count"] == 1
    assert rebuilt["bridge_subtree"]["status"] == "blocked_review_only_not_import_eligible"
    assert rebuilt["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(rebuilt)


@settings(max_examples=100)
@given(paper_id=PAPER_IDS, forbidden_key=FORBIDDEN_PAYLOAD_KEYS, unsafe_flag=UNSAFE_FLAG_KEYS)
def test_forbidden_payload_keys_and_unsafe_flags_are_redacted_and_counted(
    paper_id: str, forbidden_key: str, unsafe_flag: str
) -> None:
    manifest = _manifest(paper_id)
    manifest["source_refs"][0][forbidden_key] = "FORBIDDEN_RAW_TITLE_DO_NOT_ECHO"
    manifest["citation_links"][0]["target_ref"]["reference"] = "FORBIDDEN_RAW_REFERENCE_DO_NOT_ECHO"
    if unsafe_flag in {
        "trusted_kg_import_allowed",
        "ladybugdb_written",
        "production_import_attempted",
        "model_outputs_included",
        "raw_payloads_included",
    }:
        manifest.setdefault("safety_flags", {})[unsafe_flag] = True
    else:
        manifest["dedup_candidates"][0][unsafe_flag] = True

    rebuilt = build_article_links_dedup_manifest(manifest)
    serialized = json.dumps(rebuilt, sort_keys=True)

    assert "forbidden_payload_key" in _diagnostic_codes(rebuilt)
    assert any(code.startswith("unsafe_import_flag_true:") for code in _diagnostic_codes(rebuilt))
    assert rebuilt["summary"]["diagnostic_counts"]["forbidden_payload_detection_count"] >= 2
    assert rebuilt["summary"]["diagnostic_counts"]["unsafe_authorization_count"] == 1
    assert "FORBIDDEN_RAW_TITLE_DO_NOT_ECHO" not in serialized
    assert "FORBIDDEN_RAW_REFERENCE_DO_NOT_ECHO" not in serialized
    assert rebuilt["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(rebuilt)


@settings(max_examples=60)
@given(paper_id=PAPER_IDS)
def test_missing_anchor_and_span_coverage_counters_are_linear_over_records(paper_id: str) -> None:
    manifest = _manifest(paper_id, metadata_signal_count=2)
    manifest["page_index_refs"]["anchor_ids"] = []
    manifest["citation_links"][0]["source_page_index_anchor_id"] = ""
    manifest["citation_links"][0]["source_span_ids"] = [""]
    manifest["metadata_signals"][0]["source_page_index_anchor_id"] = ""
    manifest["metadata_signals"][0]["source_span_id"] = ""

    rebuilt = build_article_links_dedup_manifest(manifest)
    anchor_coverage = rebuilt["summary"]["page_index_anchor_coverage"]
    span_coverage = rebuilt["summary"]["source_span_coverage"]

    assert anchor_coverage["required_anchor_ref_count"] == 5
    assert anchor_coverage["missing_anchor_ref_count"] == 2
    assert span_coverage["required_source_span_ref_count"] == 5
    assert span_coverage["missing_source_span_ref_count"] == 2
    assert rebuilt["summary"]["diagnostic_counts"]["missing_page_index_anchor_count"] == 2
    assert rebuilt["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(rebuilt)
