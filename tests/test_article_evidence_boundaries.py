"""Contracts for M025 separated article evidence boundary artifacts.

S07 keeps assets, tables, links, and identity evidence separate from chunks while
preserving stable provenance references. These fixtures intentionally contain
only metadata and identifiers: no article text, binary payloads, embeddings,
model output, graph-import authorizations, or production-write claims.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_evidence_boundaries_v00_01"
EXPECTED_EVIDENCE_TYPES = {"assets", "tables", "links", "identity"}
EXPECTED_SCHEMA_PREFIX = "m025-article-evidence-"
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
    "cell_text",
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
}
FORBIDDEN_TEXT_FRAGMENTS = (
    "%PDF-",
    "data:image/",
    "base64,",
    "OPENAI_API_KEY",
    "sk-",
    "Graph-Guided Retrieval for Scientific Agents",
    "Recursive Language Models are",
    '"trusted_kg_import_allowed": true',
    '"ladybugdb_written": true',
    '"production_import_attempted": true',
    '"import_eligible": true',
    '"promoted_to_fact": true',
)
REQUIRED_FALSE_FLAGS = {
    "raw_payloads_included",
    "trusted_kg_import_allowed",
    "ladybugdb_written",
    "production_import_attempted",
}


def _load_fixture(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _walk_keys(value: Any) -> list[str]:
    if isinstance(value, dict):
        return [str(key) for key in value] + [nested for child in value.values() for nested in _walk_keys(child)]
    if isinstance(value, list):
        return [nested for child in value for nested in _walk_keys(child)]
    return []


def _walk_dicts(value: Any) -> list[dict[str, Any]]:
    if isinstance(value, dict):
        return [value] + [nested for child in value.values() for nested in _walk_dicts(child)]
    if isinstance(value, list):
        return [nested for child in value for nested in _walk_dicts(child)]
    return []


def _assert_metadata_safe(payload: dict[str, Any]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    for fragment in FORBIDDEN_TEXT_FRAGMENTS:
        assert fragment not in serialized
    assert not (set(_walk_keys(payload)) & FORBIDDEN_EXACT_KEYS)


def _assert_import_flags_fail_closed(payload: dict[str, Any]) -> None:
    assert payload["import_eligible_count"] == 0
    assert payload["promoted_to_fact_count"] == 0
    safety_flags = payload["safety_flags"]
    assert safety_flags["metadata_only"] is True
    assert safety_flags["review_only"] is True
    for flag in REQUIRED_FALSE_FLAGS:
        assert safety_flags[flag] is False
    for obj in _walk_dicts(payload):
        if "import_eligible" in obj:
            assert obj["import_eligible"] is False
        if "promoted_to_fact" in obj:
            assert obj["promoted_to_fact"] is False


def test_fixture_directory_defines_all_separated_evidence_types() -> None:
    payloads = [_load_fixture(path) for path in sorted(FIXTURES_DIR.glob("*.json"))]

    assert {payload["evidence_type"] for payload in payloads} == EXPECTED_EVIDENCE_TYPES
    assert len(payloads) == len(EXPECTED_EVIDENCE_TYPES)


def test_evidence_boundary_fixtures_are_metadata_safe_and_fail_closed() -> None:
    for path in sorted(FIXTURES_DIR.glob("*.json")):
        payload = _load_fixture(path)

        assert payload["schema_version"].startswith(EXPECTED_SCHEMA_PREFIX)
        assert payload["schema_version"].endswith(".v00.01")
        assert payload["article_ref"]
        assert payload["source_ref"]["source_id"]
        assert payload["source_ref"]["article_path"].endswith("article.json")
        assert isinstance(payload["chunk_refs"], list)
        assert payload["summary"]["item_count"] == len(payload["items"])
        assert payload["summary"]["unsupported_type_count"] >= 0
        assert payload["summary"]["diagnostic_count"] == len(payload["diagnostics"])
        _assert_metadata_safe(payload)
        _assert_import_flags_fail_closed(payload)


def test_items_reference_article_source_element_and_chunk_identity_without_payloads() -> None:
    for path in sorted(FIXTURES_DIR.glob("*.json")):
        payload = _load_fixture(path)
        article_ref = payload["article_ref"]
        known_chunk_ids = {chunk_ref["chunk_id"] for chunk_ref in payload["chunk_refs"]}

        for item in payload["items"]:
            assert any(str(value).startswith(article_ref) for value in item.values() if isinstance(value, str))
            assert "source_span_id" in item or "source_span_ids" in item
            assert "element_id" in item or "source_element_id" in item
            for chunk_id in item.get("chunk_ids", []):
                assert chunk_id in known_chunk_ids
            assert item.get("raw_text_embedded") is False
