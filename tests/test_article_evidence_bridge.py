"""Contract tests for the M024 article evidence bridge."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from arxiv_archive.article_evidence_bridge import (
    ARTICLE_EVIDENCE_BUNDLE_SCHEMA_VERSION,
    ARTICLE_EVIDENCE_DIAGNOSTICS_SCHEMA_VERSION,
    ARTICLE_EVIDENCE_RUN_SCHEMA_VERSION,
    ArticleEvidenceReplayError,
    attach_assets_summary,
    attach_links_dedup_summary,
    attach_page_index_summary,
    attach_retrieval_table_benchmark_summary,
    build_article_evidence_bundle,
    build_article_evidence_bundle_from_load_events,
    build_article_evidence_run_summary,
    build_article_evidence_run_summary_from_load_events,
    replay_input_hashes,
    replay_input_source_ids,
    to_json,
    to_redacted_dict,
    validate_article_evidence_bundle,
    validate_article_load_events,
)
from arxiv_archive.article_loader import ArticleLoadSource, load_article_source
from arxiv_archive.article_assets import build_article_asset_manifest
from arxiv_archive.article_page_index import build_article_page_index_from_structure

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_loader"
ARTICLE_STRUCTURE_FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_artifacts"
PAGE_INDEX_FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_page_index"
LINKS_DEDUP_FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_links_dedup"
RETRIEVAL_TABLES_FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_retrieval_tables"

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


def _read_jsonl(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def _load_structure_fixture(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _basic_page_index() -> dict[str, object]:
    return build_article_page_index_from_structure(
        _load_structure_fixture(ARTICLE_STRUCTURE_FIXTURES_DIR / "basic_article_structure.json")
    )


def _fallback_page_index() -> dict[str, object]:
    structure = _load_structure_fixture(ARTICLE_STRUCTURE_FIXTURES_DIR / "basic_article_structure.json")
    structure["sections"] = []
    structure["artifact_placeholders"] = []
    structure["safe_spans"] = []
    return build_article_page_index_from_structure(structure)


def _blocked_page_index() -> dict[str, object]:
    return build_article_page_index_from_structure(
        _load_structure_fixture(PAGE_INDEX_FIXTURES_DIR / "malformed_structure.json")
    )


def _assert_no_forbidden_bridge_payload(payload: dict[str, object]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    for forbidden in FORBIDDEN_SNIPPETS:
        assert forbidden not in serialized
    assert not (set(_walk_keys(payload)) & FORBIDDEN_EXACT_KEYS)


def _page_index_attached_payload(tmp_path: Path, page_index: dict[str, object]) -> dict[str, object]:
    bundle = build_article_evidence_bundle(
        _mixed_loader_results(tmp_path),
        paper_id="2605.bridge",
        run_id="m024-page-index-bridge-test",
    )
    return attach_page_index_summary(
        bundle,
        page_index,
        manifest_path="artifacts/page-index-manifest.json",
        manifest_sha256="a" * 64,
    )


def _links_dedup_manifest_for_bridge(tmp_path: Path, *, unsafe: bool = False) -> tuple[dict[str, object], dict[str, object]]:
    from arxiv_archive.article_links_dedup import build_article_links_dedup_manifest

    bundle = build_article_evidence_bundle(
        _mixed_loader_results(tmp_path),
        paper_id="2605.bridge",
        run_id="m024-links-dedup-bridge-test",
    ).to_redacted_dict()
    source_ref = dict(bundle["source_refs"][0])
    source_id = str(source_ref["source_id"])
    manifest = json.loads((LINKS_DEDUP_FIXTURES_DIR / "minimal_manifest.json").read_text(encoding="utf-8"))
    manifest["paper_id"] = "2605.bridge"
    manifest["run_id"] = "m024-links-dedup-bridge-test"
    manifest["source_refs"] = [source_ref]
    manifest["page_index_refs"] = {
        "schema_version": "m024-article-page-index.v1",
        "manifest_path": "artifacts/page-index-manifest.json",
        "manifest_sha256": "a" * 64,
        "node_ids": ["2605.bridge:page-index:section:results"],
        "anchor_ids": ["2605.bridge:page-index-anchor:citation-0001"],
    }
    for link in manifest["citation_links"] + manifest["structural_links"]:
        link["source_page_index_anchor_id"] = "2605.bridge:page-index-anchor:citation-0001"
        link["source_span_ids"] = ["2605.bridge:span:citation-0001"]
    for signal in manifest["metadata_signals"]:
        signal["source_page_index_anchor_id"] = "2605.bridge:page-index-anchor:citation-0001"
        signal["source_span_id"] = "2605.bridge:span:citation-0001"
    if unsafe:
        manifest["source_refs"][0]["source_path"] = ""
        manifest["source_refs"][0]["sha256"] = "not-a-sha"
        manifest["paper_id"] = "wrong-paper"
        manifest["citation_links"][0]["raw_text"] = "FORBIDDEN_LINK_PAYLOAD"
        manifest["import_eligible_count"] = 7
        manifest["safety_flags"]["trusted_kg_import_allowed"] = True
    return bundle, build_article_links_dedup_manifest(manifest)


def _asset_manifest_for_bridge(tmp_path: Path, *, unsafe: bool = False) -> tuple[dict[str, object], dict[str, object]]:
    bundle = build_article_evidence_bundle(
        _mixed_loader_results(tmp_path),
        paper_id="2605.bridge",
        run_id="m024-assets-bridge-test",
    ).to_redacted_dict()
    source_ref = bundle["source_refs"][0]
    source_id = str(source_ref["source_id"])
    span = {
        "span_id": "2605.bridge:span:figure-0001",
        "source_id": source_id,
        "coordinate_space": "page_bbox",
        "char_start": None,
        "char_end": None,
        "page_start": 2,
        "page_end": 2,
        "bbox": [72.0, 120.0, 468.0, 360.0],
        "span_hash": "1" * 64,
        "raw_text_embedded": False,
    }
    manifest = build_article_asset_manifest(
        {
            "paper_id": "2605.bridge",
            "run_id": "m024-assets-bridge-test",
            "source_refs": [source_ref],
            "page_index": {
                "schema_version": "m024-page-index.v1",
                "manifest_path": "artifacts/page-index-manifest.json",
                "manifest_sha256": "a" * 64,
                "nodes": [
                    {
                        "node_id": "2605.bridge:page-index:artifact:figure:0001",
                        "paper_id": "2605.bridge",
                        "node_type": "artifact",
                        "summary": {"artifact_type": "figure"},
                        "source_ref_ids": [source_id],
                        "source_span": span,
                        "anchor_ids": ["2605.bridge:page-index-anchor:figure-0001"],
                        "import_eligible": False,
                        "promoted_to_fact": False,
                    }
                ],
                "anchors": [
                    {
                        "anchor_id": "2605.bridge:page-index-anchor:figure-0001",
                        "node_id": "2605.bridge:page-index:artifact:figure:0001",
                        "source_id": source_id,
                    }
                ],
            },
            "asset_placeholders": [
                {
                    "source_asset_ref": "figure:1",
                    "asset_type": "figure",
                    "source_file_id": source_id,
                    "page_index_node_id": "2605.bridge:page-index:artifact:figure:0001",
                    "page_index_anchor_id": "2605.bridge:page-index-anchor:figure-0001",
                    "preservation_state": "source_linked",
                    "interpretation_status": "not_interpreted",
                    "source_span": span,
                    **({"import_eligible": True, "caption_text": "FORBIDDEN_ASSET_CAPTION"} if unsafe else {}),
                }
            ],
        }
    )
    return bundle, manifest


def _combined_s01_log_events(tmp_path: Path):
    paper_id = "2605.bridge"
    log_path = tmp_path / "s01-load-events.jsonl"
    results = [
        load_article_source(
            ArticleLoadSource(FIXTURES_DIR / "structured_paper.md", paper_id=paper_id, source_type="markdown"),
            log_path=log_path,
        ),
        load_article_source(
            ArticleLoadSource(FIXTURES_DIR / "minimal.pdf", paper_id=paper_id, source_type="pdf"),
            log_path=log_path,
        ),
        load_article_source(
            ArticleLoadSource(FIXTURES_DIR / "arxiv_landing_only.md", paper_id=paper_id, source_type="markdown"),
            log_path=log_path,
        ),
    ]
    return results, _read_jsonl(log_path)


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

    assert set(payload["subtrees"]) == {"raw", "normalized", "page_index", "assets", "links_dedup", "retrieval", "staging", "metrics"}
    assert payload["subtrees"]["raw"]["status"] == "metadata_only"
    assert payload["subtrees"]["normalized"]["status"] == "review_only"
    assert payload["subtrees"]["page_index"]["status"] == "not_attempted"
    assert payload["subtrees"]["assets"]["status"] == "not_attempted"
    assert payload["subtrees"]["links_dedup"] == {
        "status": "not_attempted",
        "record_count": 0,
        "citation_link_count": 0,
        "structural_link_count": 0,
        "metadata_signal_count": 0,
        "dedup_candidate_count": 0,
        "import_eligible_count": 0,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }
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


def test_replays_s01_metadata_only_jsonl_events_into_equivalent_bridge_bundle(tmp_path: Path) -> None:
    results, events = _combined_s01_log_events(tmp_path)

    direct_payload = build_article_evidence_bundle(
        results,
        paper_id="2605.bridge",
        run_id="m024-replay-test",
    ).to_redacted_dict()
    replay_payload = build_article_evidence_bundle_from_load_events(
        events,
        paper_id="2605.bridge",
        run_id="m024-replay-test",
    ).to_redacted_dict()

    assert [event["event"] for event in events] == [
        "source.load_started",
        "source.load_completed",
        "source.load_started",
        "source.load_completed",
        "source.load_started",
        "source.load_failed",
    ]
    assert validate_article_load_events(events, paper_id="2605.bridge") == []
    assert replay_payload["source_refs"] == direct_payload["source_refs"]
    assert replay_payload["summary"] == direct_payload["summary"]
    assert replay_payload["bundle_id"] == direct_payload["bundle_id"]
    assert validate_article_evidence_bundle(replay_payload) == []

    serialized = json.dumps(replay_payload, sort_keys=True)
    for forbidden in FORBIDDEN_SNIPPETS:
        assert forbidden not in serialized
    assert not (set(_walk_keys(replay_payload)) & FORBIDDEN_EXACT_KEYS)


def test_replay_run_summary_records_redacted_input_fingerprints_and_no_import_claims(tmp_path: Path) -> None:
    results, events = _combined_s01_log_events(tmp_path)
    bundle = build_article_evidence_bundle_from_load_events(
        events,
        paper_id="2605.bridge",
        run_id="m024-replay-summary-test",
    )

    run_summary = build_article_evidence_run_summary_from_load_events(
        run_id="m024-replay-summary-test",
        bundles=[bundle],
        events=events,
        output_paths={"bundle": "redacted-bundle.json", "summary": "run-summary.json"},
    )

    assert run_summary["schema_version"] == ARTICLE_EVIDENCE_RUN_SCHEMA_VERSION
    assert run_summary["bundle_count"] == 1
    assert run_summary["source_count"] == 3
    assert run_summary["outcome_counts"] == {"failed": 1, "loaded": 1, "loaded_metadata_only": 1}
    assert run_summary["input_source_ids"] == replay_input_source_ids(events)
    assert run_summary["input_hashes"] == replay_input_hashes(events)
    assert len(run_summary["input_hashes"]) == len(events)
    assert all(value.startswith("source-load-event:") for value in run_summary["input_hashes"])
    assert {result.source_id for result in results} <= set(run_summary["input_source_ids"])
    assert run_summary["import_eligible_count"] == 0
    assert run_summary["promoted_to_fact_count"] == 0
    assert run_summary["production_import_attempted"] is False
    assert run_summary["ladybugdb_written"] is False
    assert run_summary["output_paths"] == {"bundle": "redacted-bundle.json", "summary": "run-summary.json"}


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


def test_page_index_summary_attaches_valid_manifest_as_metadata_only_subtree(tmp_path: Path) -> None:
    payload = _page_index_attached_payload(tmp_path, _basic_page_index())
    subtree = payload["subtrees"]["page_index"]

    assert subtree["status"] == "metadata_only"
    assert subtree["review_only"] is True
    assert subtree["record_count"] == 6
    assert subtree["node_count"] == 6
    assert subtree["anchor_count"] == 7
    assert subtree["diagnostic_count"] == 0
    assert subtree["diagnostic_counts_by_code"] == {}
    assert subtree["blocker_count"] == 0
    assert subtree["manifest"] == {
        "path": "artifacts/page-index-manifest.json",
        "sha256": "a" * 64,
        "schema_version": "m024-article-page-index.v1",
        "diagnostics_schema_version": "m024-article-page-index-diagnostics.v1",
        "builder": "redacted_article_structure_page_index_v1",
        "paper_id": "fixture-paper-0001",
    }
    assert subtree["source_provenance"]["source_count"] == 3
    assert subtree["source_provenance"]["outcome_counts"] == {"failed": 1, "loaded": 1, "loaded_metadata_only": 1}
    assert subtree["source_provenance"]["failure_counts"] == {"no_substantive_body": 1}
    assert subtree["graph_import_claim"] is False
    assert subtree["trusted_kg_import_allowed"] is False
    assert subtree["production_import_attempted"] is False
    assert subtree["ladybugdb_written"] is False
    assert subtree["import_eligible_count"] == 0
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_page_index_summary_attaches_fallback_manifest_as_review_only(tmp_path: Path) -> None:
    payload = _page_index_attached_payload(tmp_path, _fallback_page_index())
    subtree = payload["subtrees"]["page_index"]

    assert subtree["status"] == "review_only"
    assert subtree["record_count"] == 1
    assert subtree["anchor_count"] == 0
    assert subtree["fallback_count"] == 1
    assert subtree["diagnostic_count"] == 1
    assert subtree["diagnostic_counts_by_code"] == {"no_sections_fallback": 1}
    assert subtree["blocker_count"] == 0
    assert subtree["import_eligible_count"] == 0
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_page_index_summary_attaches_blocked_manifest_as_blocked_review_only(tmp_path: Path) -> None:
    payload = _page_index_attached_payload(tmp_path, _blocked_page_index())
    subtree = payload["subtrees"]["page_index"]

    assert subtree["status"] == "blocked"
    assert subtree["record_count"] > 0
    assert subtree["missing_parent_count"] == 2
    assert subtree["missing_span_count"] == 2
    assert subtree["blocker_count"] >= 1
    assert subtree["diagnostic_counts_by_code"]["missing_parent"] >= 1
    assert subtree["diagnostic_counts_by_code"]["forbidden_payload_key"] >= 1
    assert subtree["trusted_kg_import_allowed"] is False
    assert subtree["import_eligible_count"] == 0
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_page_index_summary_forces_unsafe_import_mutation_fail_closed(tmp_path: Path) -> None:
    page_index = _basic_page_index()
    page_index["import_eligible_count"] = 9
    page_index["production_import_attempted"] = True
    page_index["bridge_subtree"]["trusted_kg_import_allowed"] = True

    payload = _page_index_attached_payload(tmp_path, page_index)
    subtree = payload["subtrees"]["page_index"]

    assert subtree["status"] == "blocked"
    assert subtree["import_eligible_count"] == 0
    assert subtree["production_import_attempted"] is False
    assert subtree["trusted_kg_import_allowed"] is False
    assert subtree["diagnostic_counts_by_code"]["import_eligible_count_nonzero"] == 1
    assert subtree["diagnostic_counts_by_code"]["unsafe_import_flag_true:production_import_attempted"] == 1
    assert subtree["diagnostic_counts_by_code"]["unsafe_import_flag_true:trusted_kg_import_allowed"] == 1
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_page_index_summary_blocks_missing_source_path_or_hash_without_raw_payload(tmp_path: Path) -> None:
    bundle = build_article_evidence_bundle(
        _mixed_loader_results(tmp_path),
        paper_id="2605.bridge",
        run_id="m024-page-index-source-negative-test",
    ).to_redacted_dict()
    bundle["source_refs"][0]["source_path"] = ""
    bundle["source_refs"][1]["sha256"] = None

    payload = attach_page_index_summary(bundle, _basic_page_index())
    subtree = payload["subtrees"]["page_index"]
    bridge_codes = subtree["diagnostic_counts_by_code"]

    assert subtree["status"] == "blocked"
    assert bridge_codes["page_index_missing_source_path"] == 1
    assert bridge_codes["page_index_missing_source_hash"] == 1
    assert subtree["source_provenance"]["source_count"] == 3
    assert "" not in subtree["source_provenance"]["source_paths"]
    assert validate_article_evidence_bundle(payload)
    _assert_no_forbidden_bridge_payload(payload)


def test_assets_summary_attaches_metadata_only_counts_and_validates_cleanly(tmp_path: Path) -> None:
    bundle, manifest = _asset_manifest_for_bridge(tmp_path)

    payload = attach_assets_summary(
        bundle,
        manifest,
        manifest_path="artifacts/article-assets-manifest.json",
        manifest_sha256="b" * 64,
    )
    subtree = payload["subtrees"]["assets"]

    assert subtree["status"] == "review_only"
    assert subtree["record_count"] == 1
    assert subtree["asset_counts_by_type"] == {"figure": 1}
    assert subtree["preservation_state_counts"] == {"source_linked": 1}
    assert subtree["interpretation_status_counts"] == {"not_interpreted": 1}
    assert subtree["blocker_count"] == 0
    assert subtree["diagnostic_count"] == 0
    assert subtree["hash_coverage_rate"] == 1.0
    assert subtree["page_index_anchor_coverage_rate"] == 1.0
    assert subtree["manifest"] == {
        "path": "artifacts/article-assets-manifest.json",
        "sha256": "b" * 64,
        "schema_version": "m024-article-assets.v1",
        "diagnostics_schema_version": "m024-article-assets-diagnostics.v1",
        "builder": "metadata_only_article_assets_v1",
        "paper_id": "2605.bridge",
    }
    assert "assets" not in subtree
    assert "source_asset_ref" not in subtree
    assert subtree["trusted_kg_import_allowed"] is False
    assert subtree["production_import_attempted"] is False
    assert subtree["ladybugdb_written"] is False
    assert subtree["import_eligible_count"] == 0
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_assets_summary_blocks_manifest_diagnostics_without_copying_records(tmp_path: Path) -> None:
    bundle, manifest = _asset_manifest_for_bridge(tmp_path, unsafe=True)

    payload = attach_assets_summary(
        bundle,
        manifest,
        manifest_path="artifacts/article-assets-manifest.json",
        manifest_sha256="b" * 64,
    )
    subtree = payload["subtrees"]["assets"]

    assert subtree["status"] == "blocked"
    assert subtree["blocker_count"] > 0
    assert subtree["diagnostic_counts_by_code"]["forbidden_payload_key"] >= 1
    assert subtree["diagnostic_counts_by_code"]["unsafe_import_eligible_flag"] >= 1
    assert subtree["import_eligible_count"] == 0
    assert subtree["trusted_kg_import_allowed"] is False
    assert "FORBIDDEN_ASSET_CAPTION" not in json.dumps(subtree, sort_keys=True)
    assert "caption_text" not in json.dumps(subtree, sort_keys=True)
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_assets_summary_blocks_missing_manifest_provenance(tmp_path: Path) -> None:
    bundle, manifest = _asset_manifest_for_bridge(tmp_path)

    payload = attach_assets_summary(bundle, manifest)
    subtree = payload["subtrees"]["assets"]

    assert subtree["status"] == "blocked"
    assert subtree["manifest"]["path"] is None
    assert subtree["manifest"]["sha256"] is None
    assert subtree["diagnostic_counts_by_code"] == {
        "assets_missing_manifest_path": 1,
        "assets_missing_manifest_sha256": 1,
    }
    assert validate_article_evidence_bundle(payload) == []


def test_assets_summary_is_deterministic_and_aggregate_only(tmp_path: Path) -> None:
    bundle, manifest = _asset_manifest_for_bridge(tmp_path)

    first = attach_assets_summary(bundle, manifest, "artifacts/article-assets-manifest.json", "b" * 64)
    second = attach_assets_summary(bundle, manifest, "artifacts/article-assets-manifest.json", "b" * 64)

    assert first["subtrees"]["assets"] == second["subtrees"]["assets"]
    serialized = json.dumps(first["subtrees"]["assets"], sort_keys=True)
    assert "asset_id" not in serialized
    assert "source_asset_ref" not in serialized
    assert "caption_text" not in serialized
    assert "table_text" not in serialized
    assert "image_bytes" not in serialized
    assert validate_article_evidence_bundle(first) == []


def test_assets_payload_bearing_bridge_subtree_fails_bundle_validation(tmp_path: Path) -> None:
    bundle, manifest = _asset_manifest_for_bridge(tmp_path)
    payload = attach_assets_summary(
        bundle,
        manifest,
        manifest_path="artifacts/article-assets-manifest.json",
        manifest_sha256="b" * 64,
    )
    payload["subtrees"]["assets"]["caption_text"] = "raw caption must not enter bridge"
    payload["subtrees"]["assets"]["import_eligible_count"] = 1

    diagnostics = validate_article_evidence_bundle(payload)
    codes = {diagnostic["code"] for diagnostic in diagnostics}

    assert "forbidden_payload_key" in codes
    assert "subtree_import_eligible_count_nonzero" in codes


def test_links_dedup_summary_attaches_review_only_aggregate_counts(tmp_path: Path) -> None:
    bundle, manifest = _links_dedup_manifest_for_bridge(tmp_path)

    payload = attach_links_dedup_summary(
        bundle,
        manifest,
        manifest_path="artifacts/article-links-dedup-manifest.json",
        manifest_sha256="c" * 64,
    )
    subtree = payload["subtrees"]["links_dedup"]

    assert subtree["status"] == "review_only"
    assert subtree["review_only"] is True
    assert subtree["record_count"] == 7
    assert subtree["citation_link_count"] == 1
    assert subtree["structural_link_count"] == 1
    assert subtree["metadata_signal_count"] == 4
    assert subtree["dedup_candidate_count"] == 1
    assert subtree["link_family_counts"] == {
        "citation": 1,
        "structural": 1,
        "metadata_signal": 4,
        "dedup_candidate": 1,
    }
    assert subtree["metadata_signal_counts"] == {
        "arxiv_id": 1,
        "content_hash": 1,
        "doi": 1,
        "url": 1,
    }
    assert subtree["dedup_decision_counts"] == {"candidate_same_work_review_required": 1}
    assert subtree["blocker_count"] == 0
    assert subtree["diagnostic_count"] == 0
    assert subtree["manifest"] == {
        "path": "artifacts/article-links-dedup-manifest.json",
        "sha256": "c" * 64,
        "schema_version": "m024-article-links-dedup.v1",
        "paper_id": "2605.bridge",
        "run_id": "m024-links-dedup-bridge-test",
    }
    assert subtree["source_provenance"]["bundle_source_count"] == 3
    assert subtree["source_provenance"]["manifest_source_count"] == 1
    assert subtree["page_index_provenance"]["anchor_ref_count"] == 1
    assert subtree["graph_import_claim"] is False
    assert subtree["trusted_kg_import_allowed"] is False
    assert subtree["production_import_attempted"] is False
    assert subtree["ladybugdb_written"] is False
    assert subtree["import_eligible_count"] == 0
    assert "citation_links" not in subtree
    assert "metadata_signals" not in subtree
    assert "dedup_candidates" not in subtree
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_links_dedup_summary_blocks_unsafe_or_mismatched_manifest_without_copying_payloads(tmp_path: Path) -> None:
    bundle, manifest = _links_dedup_manifest_for_bridge(tmp_path, unsafe=True)

    payload = attach_links_dedup_summary(
        bundle,
        manifest,
        manifest_path="artifacts/article-links-dedup-manifest.json",
        manifest_sha256="c" * 64,
    )
    subtree = payload["subtrees"]["links_dedup"]
    codes = subtree["diagnostic_counts_by_code"]

    assert subtree["status"] == "blocked"
    assert subtree["blocker_count"] > 0
    assert codes["forbidden_payload_detection_count"] == 1
    assert codes["malformed_source_ref_count"] == 1
    assert codes["links_dedup_paper_id_mismatch"] == 1
    assert codes["links_dedup_missing_source_path"] == 1
    assert codes["links_dedup_missing_source_hash"] == 1
    assert subtree["forbidden_payload_detection_count"] == 1
    assert subtree["unsafe_authorization_count"] >= 1
    assert subtree["import_eligible_count"] == 0
    assert subtree["trusted_kg_import_allowed"] is False
    assert "FORBIDDEN_LINK_PAYLOAD" not in json.dumps(subtree, sort_keys=True)
    assert "raw_text" not in json.dumps(subtree, sort_keys=True)
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_links_dedup_summary_blocks_missing_manifest_provenance(tmp_path: Path) -> None:
    bundle, manifest = _links_dedup_manifest_for_bridge(tmp_path)

    payload = attach_links_dedup_summary(bundle, manifest)
    subtree = payload["subtrees"]["links_dedup"]

    assert subtree["status"] == "blocked"
    assert subtree["manifest"]["path"] is None
    assert subtree["manifest"]["sha256"] is None
    assert subtree["diagnostic_counts_by_code"]["links_dedup_missing_manifest_path"] == 1
    assert subtree["diagnostic_counts_by_code"]["links_dedup_missing_manifest_sha256"] == 1
    assert validate_article_evidence_bundle(payload) == []


def test_links_dedup_payload_bearing_bridge_subtree_fails_bundle_validation(tmp_path: Path) -> None:
    bundle, manifest = _links_dedup_manifest_for_bridge(tmp_path)
    payload = attach_links_dedup_summary(
        bundle,
        manifest,
        manifest_path="artifacts/article-links-dedup-manifest.json",
        manifest_sha256="c" * 64,
    )
    payload["subtrees"]["links_dedup"]["raw_text"] = "raw link payload must not enter bridge"
    payload["subtrees"]["links_dedup"]["import_eligible_count"] = 1
    payload["subtrees"]["links_dedup"]["source_provenance"]["manifest_source_ids"] = ["not-in-bundle"]

    diagnostics = validate_article_evidence_bundle(payload)
    codes = {diagnostic["code"] for diagnostic in diagnostics}

    assert "forbidden_payload_key" in codes
    assert "subtree_import_eligible_count_nonzero" in codes
    assert "links_dedup_source_ref_not_in_bundle" in codes



def _retrieval_table_manifest_for_bridge(tmp_path: Path) -> tuple[dict[str, object], dict[str, object]]:
    from arxiv_archive.article_retrieval_tables import build_article_retrieval_table_manifest

    bundle = build_article_evidence_bundle(
        _mixed_loader_results(tmp_path),
        paper_id="2605.bridge",
        run_id="m024-retrieval-table-bridge-test",
    ).to_redacted_dict()
    manifest = json.loads((RETRIEVAL_TABLES_FIXTURES_DIR / "minimal_manifest.json").read_text(encoding="utf-8"))
    source_refs = [dict(bundle["source_refs"][0]), dict(bundle["source_refs"][1])]
    manifest["paper_id"] = "2605.bridge"
    manifest["run_id"] = "m024-retrieval-table-bridge-test"
    manifest["source_refs"] = source_refs
    normalized_source_id = str(source_refs[0]["source_id"])
    pdf_source_id = str(source_refs[1]["source_id"])
    for unit in manifest["retrieval_units"]:
        if unit["unit_family"] == "table_candidate_context_unit":
            unit["source_ref_ids"] = [pdf_source_id]
        else:
            unit["source_ref_ids"] = [normalized_source_id]
    manifest["table_candidates"][0]["source_ref_ids"] = [pdf_source_id]
    return bundle, build_article_retrieval_table_manifest(
        paper_id=manifest["paper_id"],
        run_id=manifest["run_id"],
        source_refs=manifest["source_refs"],
        page_index_refs=manifest["page_index_refs"],
        asset_refs=manifest["asset_refs"],
        links_dedup_refs=manifest["links_dedup_refs"],
        retrieval_units=manifest["retrieval_units"],
        table_candidates=manifest["table_candidates"],
        manifest_path=manifest["manifest_path"],
    )


def test_retrieval_table_benchmark_summary_attaches_review_only_aggregate_counts(tmp_path: Path) -> None:
    bundle, manifest = _retrieval_table_manifest_for_bridge(tmp_path)

    payload = attach_retrieval_table_benchmark_summary(
        bundle,
        manifest,
        manifest_path=manifest["manifest_path"],
        manifest_sha256=manifest["manifest_sha256"],
    )
    subtree = payload["subtrees"]["retrieval"]
    metrics = payload["subtrees"]["metrics"]["retrieval_table_benchmark"]

    assert subtree["status"] == "review_only"
    assert subtree["review_only"] is True
    assert subtree["record_count"] == 4
    assert subtree["retrieval_unit_count"] == 3
    assert subtree["table_candidate_count"] == 1
    assert subtree["included_review_only_count"] == 4
    assert subtree["ranking_tie_count"] == 1
    assert subtree["source_ref_count"] == 2
    assert subtree["page_index_node_ref_count"] == 3
    assert subtree["page_index_anchor_ref_count"] == 3
    assert subtree["asset_ref_count"] == 1
    assert subtree["link_provenance_ref_count"] == 3
    assert subtree["manifest_provenance_count"] == 3
    assert subtree["diagnostic_count"] == 0
    assert subtree["diagnostic_counts_by_code"] == {}
    assert subtree["manifest"] == {
        "path": "artifacts/retrieval-table-benchmark-manifest.json",
        "sha256": "0" * 64,
        "schema_version": "m024-article-retrieval-tables.v1",
        "manifest_schema": "m024-article-retrieval-tables.v1",
        "builder": "metadata_only_article_retrieval_tables_v1",
        "paper_id": "2605.bridge",
        "run_id": "m024-retrieval-table-bridge-test",
    }
    assert subtree["source_provenance"]["bundle_source_count"] == 3
    assert subtree["source_provenance"]["manifest_source_count"] == 2
    assert subtree["source_provenance"]["source_hash_coverage_rate"] == 1.0
    assert subtree["page_index_provenance"]["node_ref_count"] == 3
    assert subtree["asset_provenance"]["asset_ref_count"] == 1
    assert subtree["links_dedup_provenance"]["metadata_signal_ref_count"] == 2
    assert subtree["links_dedup_provenance"]["dedup_candidate_ref_count"] == 1
    assert subtree["embedding_generation_attempted"] is False
    assert subtree["vector_indexing_attempted"] is False
    assert subtree["import_eligible_count"] == 0
    assert subtree["promoted_to_fact_count"] == 0
    assert subtree["production_import_attempted"] is False
    assert subtree["ladybugdb_written"] is False
    assert metrics["record_count"] == 4
    assert metrics["manifest_path"] == "artifacts/retrieval-table-benchmark-manifest.json"
    assert metrics["import_eligible_count"] == 0
    assert "retrieval_units" not in subtree
    assert "table_candidates" not in subtree
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_retrieval_table_benchmark_summary_blocks_unsafe_manifest_without_copying_payloads(tmp_path: Path) -> None:
    bundle = build_article_evidence_bundle(
        _mixed_loader_results(tmp_path),
        paper_id="2605.bridge",
        run_id="m024-retrieval-table-bridge-unsafe-test",
    ).to_redacted_dict()
    manifest = json.loads((RETRIEVAL_TABLES_FIXTURES_DIR / "unsafe_manifest.json").read_text(encoding="utf-8"))

    payload = attach_retrieval_table_benchmark_summary(
        bundle,
        manifest,
        manifest_path="artifacts/different-retrieval-table-manifest.json",
        manifest_sha256="d" * 64,
    )
    subtree = payload["subtrees"]["retrieval"]
    codes = subtree["diagnostic_counts_by_code"]

    assert subtree["status"] == "blocked"
    assert subtree["blocker_count"] > 0
    assert codes["forbidden_payload_key"] >= 1
    assert codes["unsafe_authorization"] >= 1
    assert codes["unsafe_readiness"] >= 1
    assert codes["retrieval_table_manifest_path_mismatch"] == 1
    assert codes["retrieval_table_manifest_sha256_mismatch"] == 1
    assert codes["retrieval_table_paper_id_mismatch"] == 1
    assert codes["retrieval_table_source_ref_not_in_bundle"] == 1
    assert subtree["forbidden_payload_detection_count"] >= 1
    assert subtree["unsafe_authorization_count"] >= 1
    assert subtree["unsafe_readiness_count"] >= 1
    assert subtree["import_eligible_count"] == 0
    assert subtree["trusted_kg_import_allowed"] is False
    assert subtree["production_import_attempted"] is False
    assert subtree["ladybugdb_written"] is False
    assert "FORBIDDEN_RAW_ARTICLE_TEXT_DO_NOT_ECHO" not in json.dumps(subtree, sort_keys=True)
    assert "FORBIDDEN_TABLE_TEXT_DO_NOT_ECHO" not in json.dumps(subtree, sort_keys=True)
    assert "raw_text" not in json.dumps(subtree, sort_keys=True)
    assert "table_text" not in json.dumps(subtree, sort_keys=True)
    assert validate_article_evidence_bundle(payload) == []
    _assert_no_forbidden_bridge_payload(payload)


def test_retrieval_table_benchmark_summary_blocks_missing_manifest_provenance(tmp_path: Path) -> None:
    bundle, manifest = _retrieval_table_manifest_for_bridge(tmp_path)
    manifest.pop("manifest_path")
    manifest.pop("manifest_sha256")

    payload = attach_retrieval_table_benchmark_summary(bundle, manifest)
    subtree = payload["subtrees"]["retrieval"]

    assert subtree["status"] == "blocked"
    assert subtree["manifest"]["path"] is None
    assert subtree["manifest"]["sha256"] is None
    assert subtree["diagnostic_counts_by_code"]["retrieval_table_missing_manifest_path"] == 1
    assert subtree["diagnostic_counts_by_code"]["retrieval_table_missing_manifest_sha256"] == 1
    assert validate_article_evidence_bundle(payload) == []


def test_retrieval_table_payload_bearing_bridge_subtree_fails_bundle_validation(tmp_path: Path) -> None:
    bundle, manifest = _retrieval_table_manifest_for_bridge(tmp_path)
    payload = attach_retrieval_table_benchmark_summary(
        bundle,
        manifest,
        manifest_path=manifest["manifest_path"],
        manifest_sha256=manifest["manifest_sha256"],
    )
    payload["subtrees"]["retrieval"]["raw_text"] = "raw retrieval text must not enter bridge"
    payload["subtrees"]["retrieval"]["import_eligible_count"] = 1

    diagnostics = validate_article_evidence_bundle(payload)
    codes = {diagnostic["code"] for diagnostic in diagnostics}

    assert "forbidden_payload_key" in codes
    assert "subtree_import_eligible_count_nonzero" in codes

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
    ("mutate", "expected_code"),
    [
        (lambda events: events[0].__setitem__("event", "source.fetch_completed"), "unsupported_replay_event"),
        (lambda events: events[1].pop("source_path"), "missing_source_path"),
        (lambda events: events[1].__setitem__("sha256", "not-a-sha"), "invalid_sha256"),
        (lambda events: events.append(dict(events[1])), "duplicate_terminal_source_id"),
        (lambda events: events[1].__setitem__("text", "raw article payload should be refused"), "forbidden_payload_key"),
        (lambda events: events[1].__setitem__("outcome", "failed"), "terminal_event_outcome_mismatch"),
    ],
)
def test_replay_rejects_malformed_or_payload_bearing_s01_events(tmp_path: Path, mutate, expected_code: str) -> None:
    _, events = _combined_s01_log_events(tmp_path)
    mutate(events)

    diagnostics = validate_article_load_events(events, paper_id="2605.bridge")
    assert expected_code in {diagnostic["code"] for diagnostic in diagnostics}
    assert all(diagnostic["json_path"].startswith("/events[") for diagnostic in diagnostics)
    assert "Graph-Guided Retrieval for Scientific Agents" not in json.dumps(diagnostics, sort_keys=True)

    with pytest.raises(ArticleEvidenceReplayError) as exc_info:
        build_article_evidence_bundle_from_load_events(
            events,
            paper_id="2605.bridge",
            run_id="m024-replay-negative-test",
        )
    assert expected_code in {diagnostic["code"] for diagnostic in exc_info.value.diagnostics}


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
