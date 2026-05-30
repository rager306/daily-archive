"""Contract tests for the M024 article evidence bridge."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from arxiv_archive.article_evidence_bridge import (
    ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION,
    ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION,
    ARTICLE_EVIDENCE_RUN_SCHEMA_VERSION,
    build_article_evidence_bundle,
    build_article_evidence_run_summary,
    to_json,
    to_redacted_dict,
    validate_article_evidence_bundle,
)
from arxiv_archive.article_loader import ArticleLoadSource, load_article_source

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_loader"

FORBIDDEN_SNIPPETS = [
    "%PDF-1.4",
    "Graph-Guided Retrieval for Scientific Agents",
    "Local article loading provides a reliable contract",
    "OPENAI_API_KEY",
    "sk-test-secret",
    "base64,",
    "embedding=[",
    "embeddings=[",
    "vector=[",
    "vectors=[",
    "api_key=",
    "token=",
]
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
}


def _mixed_loader_results(tmp_path: Path):
    paper_id = "2605.bridge"
    return [
        load_article_source(
            ArticleLoadSource(FIXTURES_DIR / "structured_paper.md", paper_id=paper_id, source_type="markdown"),
            log_path=tmp_path / "structured.jsonl",
        ),
        load_article_source(
            ArticleLoadSource(FIXTURES_DIR / "minimal.pdf", paper_id=paper_id, source_type="pdf"),
            log_path=tmp_path / "pdf.jsonl",
        ),
        load_article_source(
            ArticleLoadSource(FIXTURES_DIR / "arxiv_landing_only.md", paper_id=paper_id, source_type="markdown"),
            log_path=tmp_path / "landing.jsonl",
        ),
    ]


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


def test_builds_valid_mixed_outcome_bundle_from_loader_metadata_only(tmp_path: Path) -> None:
    results = _mixed_loader_results(tmp_path)

    bundle = build_article_evidence_bundle(
        results,
        paper_id="2605.bridge",
        run_id="m024-s02-test-run",
        bundle_root=tmp_path / "bundle-root",
    )
    payload = bundle.to_redacted_dict()

    assert payload["schema_version"] == ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION
    assert payload["diagnostics_schema_version"] == ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION
    assert payload["paper_id"] == "2605.bridge"
    assert payload["bundle_id"].startswith("article-evidence-bundle:")
    assert payload["bundle_root"] == str(tmp_path / "bundle-root")
    assert payload["summary"]["source_count"] == 3
    assert payload["summary"]["outcome_counts"] == {"failed": 1, "loaded": 1, "loaded_metadata_only": 1}
    assert payload["summary"]["failure_counts"] == {"no_substantive_body": 1}
    assert payload["summary"]["checksum_count"] == 3
    assert payload["summary"]["checksum_coverage_rate"] == 1.0
    assert payload["summary"]["import_eligible_count"] == 0
    assert payload["summary"]["promoted_to_fact_count"] == 0
    assert payload["summary"]["production_import_attempted"] is False
    assert payload["summary"]["ladybugdb_written"] is False

    assert set(payload["subtrees"]) == {"raw", "normalized", "page_index", "assets", "retrieval", "staging", "metrics"}
    assert payload["subtrees"]["raw"]["status"] == "metadata_only"
    assert payload["subtrees"]["normalized"]["status"] == "review_only"
    assert payload["subtrees"]["page_index"]["status"] == "not_attempted"
    assert payload["subtrees"]["assets"]["status"] == "not_attempted"
    assert payload["subtrees"]["retrieval"]["embedding_generation_attempted"] is False
    assert payload["subtrees"]["staging"]["status"] == "blocked"
    assert payload["subtrees"]["metrics"]["ladybugdb_written"] is False

    for source_ref in payload["source_refs"]:
        assert source_ref["paper_id"] == "2605.bridge"
        assert source_ref["source_id"].startswith("article-source:")
        assert source_ref["source_path"]
        assert source_ref["source_type"] in {"markdown", "pdf"}
        assert source_ref["media_type"] in {"text/markdown", "application/pdf"}
        assert source_ref["sha256"] and len(source_ref["sha256"]) == 64
        assert isinstance(source_ref["byte_size"], int) and source_ref["byte_size"] > 0
        assert source_ref["parser_name"] in {"markdown_loader", "pdf_metadata_probe"}
        assert source_ref["loader_name"] == "local_article_loader"
        assert source_ref["load_outcome"] in {"loaded", "loaded_metadata_only", "failed"}
        assert source_ref["raw_text_embedded"] is False
        assert source_ref["raw_binary_embedded"] is False

    assert validate_article_evidence_bundle(bundle) == []


def test_bundle_id_and_json_are_deterministic_for_reordered_loader_results(tmp_path: Path) -> None:
    results = _mixed_loader_results(tmp_path)

    first = build_article_evidence_bundle(results, paper_id="2605.bridge", run_id="m024-s02-test-run")
    second = build_article_evidence_bundle(list(reversed(results)), paper_id="2605.bridge", run_id="m024-s02-test-run")

    assert first.bundle_id == second.bundle_id
    assert to_json(first) == to_json(second)


def test_redacted_bundle_json_excludes_source_payloads_and_forbidden_payload_keys(tmp_path: Path) -> None:
    secret_source = tmp_path / "secret-bearing.md"
    secret_source.write_text(
        "# Local fixture\n\n"
        "This source mentions OPENAI_API_KEY=sk-test-secret1234567890 inside article text.\n"
        "The bridge must never serialize article payload text.\n",
        encoding="utf-8",
    )
    results = _mixed_loader_results(tmp_path) + [
        load_article_source(
            ArticleLoadSource(secret_source, paper_id="2605.bridge", source_type="markdown"),
            log_path=tmp_path / "secret.jsonl",
        )
    ]
    assert any(result.text and "OPENAI_API_KEY=sk-test-secret" in result.text for result in results)

    payload = build_article_evidence_bundle(results, paper_id="2605.bridge", run_id="m024-redaction-test").to_redacted_dict()
    serialized = json.dumps(payload, sort_keys=True)

    for forbidden in FORBIDDEN_SNIPPETS:
        assert forbidden not in serialized
    assert not (set(_walk_keys(payload)) & FORBIDDEN_EXACT_KEYS)
    assert validate_article_evidence_bundle(payload) == []


def test_validation_reports_stable_diagnostics_for_unsafe_or_malformed_bundle(tmp_path: Path) -> None:
    result = load_article_source(
        ArticleLoadSource(FIXTURES_DIR / "structured_paper.md", paper_id="2605.bridge", source_type="markdown"),
        log_path=tmp_path / "structured.jsonl",
    )
    payload = build_article_evidence_bundle([result], paper_id="2605.bridge", run_id="m024-negative-test").to_redacted_dict()
    payload["source_refs"][0]["load_outcome"] = "promoted"
    payload["source_refs"][0]["raw_text_embedded"] = True
    payload["summary"]["import_eligible_count"] = 1
    payload["production_import_attempted"] = True
    payload["allowed_uses"].append("trusted_kg_import")
    payload["subtrees"]["retrieval"]["status"] = "ready_for_graph"
    payload["source_refs"].append(dict(payload["source_refs"][0]))
    payload["text"] = "raw article payload should be refused"

    diagnostics = validate_article_evidence_bundle(payload)
    codes = {diagnostic["code"] for diagnostic in diagnostics}
    paths_by_code = {diagnostic["code"]: diagnostic["json_path"] for diagnostic in diagnostics}

    assert "invalid_load_outcome" in codes
    assert "source_ref_raw_text_embedded" in codes
    assert "summary_import_eligible_count_nonzero" in codes
    assert "production_import_attempted_true" in codes
    assert "trusted_import_allowed" in codes
    assert "invalid_subtree_status" in codes
    assert "duplicate_source_id" in codes
    assert "forbidden_payload_key" in codes
    assert paths_by_code["forbidden_payload_key"] == "/text"
    assert all(diagnostic["blocks_import"] is True for diagnostic in diagnostics)


def test_run_summary_preserves_fail_closed_counts_without_graph_claims(tmp_path: Path) -> None:
    bundle = build_article_evidence_bundle(
        _mixed_loader_results(tmp_path),
        paper_id="2605.bridge",
        run_id="m024-s02-test-run",
    )

    run_summary = build_article_evidence_run_summary(
        run_id="m024-s02-test-run",
        bundles=[bundle],
        output_paths={"bundle": "redacted-bundle.json"},
    )

    assert run_summary["schema_version"] == ARTICLE_EVIDENCE_RUN_SCHEMA_VERSION
    assert run_summary["bundle_schema_version"] == ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION
    assert run_summary["diagnostics_schema_version"] == ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION
    assert run_summary["paper_count"] == 1
    assert run_summary["bundle_count"] == 1
    assert run_summary["source_count"] == 3
    assert run_summary["outcome_counts"] == {"failed": 1, "loaded": 1, "loaded_metadata_only": 1}
    assert run_summary["failure_counts"] == {"no_substantive_body": 1}
    assert run_summary["import_eligible_count"] == 0
    assert run_summary["promoted_to_fact_count"] == 0
    assert run_summary["production_import_attempted"] is False
    assert run_summary["ladybugdb_written"] is False
    assert to_redacted_dict(run_summary) == run_summary


@pytest.mark.parametrize(
    ("mutation", "expected_code"),
    [
        (lambda payload: payload.pop("source_refs"), "missing_source_refs"),
        (lambda payload: payload["source_refs"][0].__setitem__("sha256", "not-a-sha"), "invalid_sha256"),
        (lambda payload: payload["source_refs"][0].__setitem__("failure_reason", "unexpected"), "unexpected_failure_reason"),
        (lambda payload: payload["safety_flags"].__setitem__("ladybugdb_written", True), "safety_flag_true:ladybugdb_written"),
    ],
)
def test_negative_validation_boundaries_are_redacted_and_path_addressable(tmp_path: Path, mutation, expected_code: str) -> None:
    result = load_article_source(
        ArticleLoadSource(FIXTURES_DIR / "structured_paper.md", paper_id="2605.bridge", source_type="markdown"),
        log_path=tmp_path / f"{expected_code}.jsonl",
    )
    payload = build_article_evidence_bundle([result], paper_id="2605.bridge", run_id="m024-boundary-test").to_redacted_dict()

    mutation(payload)

    diagnostics = validate_article_evidence_bundle(payload)
    assert expected_code in {diagnostic["code"] for diagnostic in diagnostics}
    assert all(diagnostic["json_path"].startswith("/") for diagnostic in diagnostics)
    assert "Graph-Guided Retrieval for Scientific Agents" not in json.dumps(diagnostics, sort_keys=True)
