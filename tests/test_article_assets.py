"""Contract tests for M024 metadata-only article asset preservation.

S04 defines the first-class asset preservation boundary after S01 source loading,
S02 evidence bridging, and S03 PageIndex construction. The contract is purposely
metadata-only: figures, tables, and equation images are preserved as stable
review assets with provenance and interpretation status, not as extracted image
bytes, captions, table contents, equation text, embeddings, or graph-importable
facts.
"""

from __future__ import annotations

import importlib
import json
from pathlib import Path
from copy import deepcopy

import pytest

PAPER_ID = "fixture-paper-0001"
SOURCE_ID = f"{PAPER_ID}:source:pdf"
SOURCE_SHA256 = "a" * 64
MANIFEST_SHA256 = "b" * 64

FORBIDDEN_SENTINEL = "FORBIDDEN_SENTINEL_DO_NOT_ECHO"
FORBIDDEN_PAYLOAD_KEYS = (
    "caption_text",
    "table_text",
    "equation_text",
    "image_bytes",
    "base64",
    "embedding",
    "vector",
    "payload",
)
FORBIDDEN_PAYLOAD_FRAGMENTS = tuple(f'"{key}":' for key in FORBIDDEN_PAYLOAD_KEYS) + (
    FORBIDDEN_SENTINEL,
    '"trusted_kg_import_allowed": true',
    '"ladybugdb_written": true',
    '"production_import_attempted": true',
    '"import_eligible": true',
    '"promoted_to_fact": true',
)


@pytest.fixture()
def article_assets_contract():
    """Load the canonical article assets module."""

    try:
        return importlib.import_module("research_graph.papers.assets")
    except ModuleNotFoundError as exc:
        if exc.name == "research_graph.papers.assets":
            pytest.xfail("research_graph.papers.assets is not implemented yet")
        raise


def _source_refs() -> list[dict[str, object]]:
    """S01/S02-style source references: provenance only, no payload."""

    return [
        {
            "source_id": SOURCE_ID,
            "paper_id": PAPER_ID,
            "source_path": "fixtures/articles/fixture-paper-0001.pdf",
            "source_type": "pdf",
            "source_role": "original_pdf",
            "media_type": "application/pdf",
            "sha256": SOURCE_SHA256,
            "byte_size": 12048,
            "parser_name": "pdf_metadata_loader",
            "loader_name": "article_loader",
            "load_outcome": "loaded_metadata_only",
            "raw_text_embedded": False,
            "raw_binary_embedded": False,
        }
    ]


def _page_index_refs() -> dict[str, list[dict[str, object]]]:
    """S03-style PageIndex node and anchor references used by assets."""

    return {
        "nodes": [
            {
                "node_id": f"{PAPER_ID}:page-index:artifact:figure:0001",
                "paper_id": PAPER_ID,
                "node_type": "artifact",
                "summary": {"artifact_type": "figure"},
                "source_ref_ids": [SOURCE_ID],
                "source_span": {
                    "span_id": f"{PAPER_ID}:span:figure-0001",
                    "source_id": SOURCE_ID,
                    "coordinate_space": "page_bbox",
                    "char_start": None,
                    "char_end": None,
                    "page_start": 2,
                    "page_end": 2,
                    "bbox": [72.0, 120.0, 468.0, 360.0],
                    "span_hash": "1" * 64,
                    "raw_text_embedded": False,
                },
                "anchor_ids": [f"{PAPER_ID}:page-index-anchor:figure-0001"],
                "import_eligible": False,
                "promoted_to_fact": False,
            },
            {
                "node_id": f"{PAPER_ID}:page-index:artifact:table:0001",
                "paper_id": PAPER_ID,
                "node_type": "artifact",
                "summary": {"artifact_type": "table"},
                "source_ref_ids": [SOURCE_ID],
                "source_span": {
                    "span_id": f"{PAPER_ID}:span:table-0001",
                    "source_id": SOURCE_ID,
                    "coordinate_space": "page_bbox",
                    "char_start": None,
                    "char_end": None,
                    "page_start": 3,
                    "page_end": 3,
                    "bbox": [80.0, 90.0, 500.0, 250.0],
                    "span_hash": "2" * 64,
                    "raw_text_embedded": False,
                },
                "anchor_ids": [f"{PAPER_ID}:page-index-anchor:table-0001"],
                "import_eligible": False,
                "promoted_to_fact": False,
            },
            {
                "node_id": f"{PAPER_ID}:page-index:artifact:equation:0001",
                "paper_id": PAPER_ID,
                "node_type": "artifact",
                "summary": {"artifact_type": "equation"},
                "source_ref_ids": [SOURCE_ID],
                "source_span": {
                    "span_id": f"{PAPER_ID}:span:equation-0001",
                    "source_id": SOURCE_ID,
                    "coordinate_space": "page_bbox",
                    "char_start": None,
                    "char_end": None,
                    "page_start": 4,
                    "page_end": 4,
                    "bbox": [96.0, 400.0, 420.0, 438.0],
                    "span_hash": "3" * 64,
                    "raw_text_embedded": False,
                },
                "anchor_ids": [f"{PAPER_ID}:page-index-anchor:equation-0001"],
                "import_eligible": False,
                "promoted_to_fact": False,
            },
        ],
        "anchors": [
            {
                "anchor_id": f"{PAPER_ID}:page-index-anchor:figure-0001",
                "node_id": f"{PAPER_ID}:page-index:artifact:figure:0001",
                "paper_id": PAPER_ID,
                "span_id": f"{PAPER_ID}:span:figure-0001",
                "source_id": SOURCE_ID,
                "coordinate_space": "page_bbox",
                "span_hash": "1" * 64,
                "anchor_type": "figure",
                "raw_text_embedded": False,
                "import_eligible": False,
                "promoted_to_fact": False,
            },
            {
                "anchor_id": f"{PAPER_ID}:page-index-anchor:table-0001",
                "node_id": f"{PAPER_ID}:page-index:artifact:table:0001",
                "paper_id": PAPER_ID,
                "span_id": f"{PAPER_ID}:span:table-0001",
                "source_id": SOURCE_ID,
                "coordinate_space": "page_bbox",
                "span_hash": "2" * 64,
                "anchor_type": "table",
                "raw_text_embedded": False,
                "import_eligible": False,
                "promoted_to_fact": False,
            },
            {
                "anchor_id": f"{PAPER_ID}:page-index-anchor:equation-0001",
                "node_id": f"{PAPER_ID}:page-index:artifact:equation:0001",
                "paper_id": PAPER_ID,
                "span_id": f"{PAPER_ID}:span:equation-0001",
                "source_id": SOURCE_ID,
                "coordinate_space": "page_bbox",
                "span_hash": "3" * 64,
                "anchor_type": "equation",
                "raw_text_embedded": False,
                "import_eligible": False,
                "promoted_to_fact": False,
            },
        ],
    }


def _manifest_input() -> dict[str, object]:
    refs = _page_index_refs()
    return {
        "paper_id": PAPER_ID,
        "run_id": "m024-s04-assets-contract-test",
        "source_refs": _source_refs(),
        "page_index": {
            "schema_version": "m024-article-page-index.v1",
            "manifest_path": "artifacts/page-index-manifest.json",
            "manifest_sha256": MANIFEST_SHA256,
            "nodes": refs["nodes"],
            "anchors": refs["anchors"],
            "bridge_subtree": {
                "status": "review_only_not_import_eligible",
                "trusted_kg_import_allowed": False,
                "ladybugdb_written": False,
                "production_import_attempted": False,
            },
        },
        "asset_placeholders": [
            {
                "source_asset_ref": "figure:1",
                "asset_type": "figure",
                "page_index_node_id": f"{PAPER_ID}:page-index:artifact:figure:0001",
                "page_index_anchor_id": f"{PAPER_ID}:page-index-anchor:figure-0001",
                "source_file_id": SOURCE_ID,
                "source_span_id": f"{PAPER_ID}:span:figure-0001",
                "source_span": refs["nodes"][0]["source_span"],
                "preservation_state": "placeholder_only",
                "interpretation_status": "not_interpreted",
            },
            {
                "source_asset_ref": "table:1",
                "asset_type": "table",
                "page_index_node_id": f"{PAPER_ID}:page-index:artifact:table:0001",
                "page_index_anchor_id": f"{PAPER_ID}:page-index-anchor:table-0001",
                "source_file_id": SOURCE_ID,
                "source_span_id": f"{PAPER_ID}:span:table-0001",
                "source_span": refs["nodes"][1]["source_span"],
                "preservation_state": "placeholder_only",
                "interpretation_status": "not_interpreted",
            },
            {
                "source_asset_ref": "equation:1",
                "asset_type": "equation_image",
                "page_index_node_id": f"{PAPER_ID}:page-index:artifact:equation:0001",
                "page_index_anchor_id": f"{PAPER_ID}:page-index-anchor:equation-0001",
                "source_file_id": SOURCE_ID,
                "source_span_id": f"{PAPER_ID}:span:equation-0001",
                "source_span": refs["nodes"][2]["source_span"],
                "preservation_state": "placeholder_only",
                "interpretation_status": "not_interpreted",
            },
        ],
    }


def _assert_metadata_only(payload: dict[str, object] | list[dict[str, object]]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    for fragment in FORBIDDEN_PAYLOAD_FRAGMENTS:
        assert fragment not in serialized


def _diagnostic_codes(manifest: dict[str, object]) -> set[str]:
    diagnostics = manifest.get("diagnostics", [])
    assert isinstance(diagnostics, list)
    return {str(diagnostic["code"]) for diagnostic in diagnostics}


def _diagnostic_by_code(manifest: dict[str, object], code: str) -> dict[str, object]:
    diagnostics = manifest.get("diagnostics", [])
    assert isinstance(diagnostics, list)
    for diagnostic in diagnostics:
        if diagnostic.get("code") == code:
            return diagnostic
    raise AssertionError(f"missing diagnostic code: {code}")


def test_builds_minimal_asset_manifest_with_stable_ids_and_summary(article_assets_contract) -> None:
    manifest = article_assets_contract.build_article_asset_manifest(_manifest_input())

    assert article_assets_contract.validate_article_asset_manifest(manifest) == []
    assert manifest["schema_version"] == article_assets_contract.ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION
    assert manifest["diagnostics_schema_version"] == article_assets_contract.ARTICLE_ASSET_DIAGNOSTICS_SCHEMA_VERSION
    assert manifest["paper_id"] == PAPER_ID
    assert manifest["run_id"] == "m024-s04-assets-contract-test"
    assert manifest["manifest_id"] == f"{PAPER_ID}:article-assets:manifest:v1"
    assert manifest["source_refs"] == _source_refs()
    assert manifest["page_index_manifest"] == {
        "schema_version": "m024-article-page-index.v1",
        "manifest_path": "artifacts/page-index-manifest.json",
        "manifest_sha256": MANIFEST_SHA256,
    }
    assert manifest["summary"] == {
        "asset_count": 3,
        "asset_counts_by_type": {"equation_image": 1, "figure": 1, "table": 1},
        "preservation_state_counts": {"placeholder_only": 3},
        "interpretation_status_counts": {"not_interpreted": 3},
        "source_ref_count": 1,
        "page_index_node_ref_count": 3,
        "page_index_anchor_ref_count": 3,
        "hash_coverage_rate": 1.0,
        "page_index_anchor_coverage_rate": 1.0,
        "source_span_coverage_rate": 1.0,
        "blocker_count": 0,
        "import_ineligible_count": 3,
        "diagnostic_count_by_code": {},
    }
    assert manifest["subtree"] == {
        "status": "review_only_not_import_eligible",
        "asset_count": 3,
        "blocker_count": 0,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }
    assert manifest["diagnostics"] == []
    assert manifest["import_eligible_count"] == 0
    assert manifest["promoted_to_fact_count"] == 0
    assert manifest["production_import_attempted"] is False
    assert manifest["ladybugdb_written"] is False
    assert manifest["safety_flags"] == {
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "raw_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "model_outputs_included": False,
    }
    _assert_metadata_only(manifest)


def test_asset_records_preserve_page_source_bbox_and_status_vocabularies(article_assets_contract) -> None:
    manifest = article_assets_contract.build_article_asset_manifest(_manifest_input())

    assert article_assets_contract.ALLOWED_ASSET_TYPES == frozenset({"figure", "diagram", "chart", "table", "equation_image"})
    assert article_assets_contract.ALLOWED_PRESERVATION_STATES == frozenset(
        {"placeholder_only", "source_linked", "binary_preserved", "unresolved"}
    )
    assert article_assets_contract.ALLOWED_INTERPRETATION_STATUSES == frozenset(
        {"not_interpreted", "needs_human_review", "interpretation_deferred", "not_applicable"}
    )

    asset_ids = [asset["asset_id"] for asset in manifest["assets"]]
    assert asset_ids == [
        f"{PAPER_ID}:asset:figure:0001",
        f"{PAPER_ID}:asset:table:0001",
        f"{PAPER_ID}:asset:equation-image:0001",
    ]

    figure, table, equation = manifest["assets"]
    assert figure["asset_type"] == "figure"
    assert table["asset_type"] == "table"
    assert equation["asset_type"] == "equation_image"
    for asset in manifest["assets"]:
        assert asset["paper_id"] == PAPER_ID
        assert asset["asset_id"].startswith(f"{PAPER_ID}:asset:")
        assert asset["source_file_id"] == SOURCE_ID
        assert asset["source_sha256"] == SOURCE_SHA256
        assert asset["page_index_node_id"].startswith(f"{PAPER_ID}:page-index:artifact:")
        assert asset["page_index_anchor_id"].startswith(f"{PAPER_ID}:page-index-anchor:")
        assert asset["preservation_state"] == "placeholder_only"
        assert asset["interpretation_status"] == "not_interpreted"
        assert asset["source_span"]["coordinate_space"] == "page_bbox"
        assert asset["source_span"]["page_start"] is not None
        assert asset["source_span"]["page_end"] is not None
        assert len(asset["source_span"]["bbox"]) == 4
        assert len(asset["source_span"]["span_hash"]) == 64
        assert asset["source_span"]["raw_text_embedded"] is False
        assert asset["raw_binary_embedded"] is False
        assert asset["base64_embedded"] is False
        assert asset["import_eligible"] is False
        assert asset["promoted_to_fact"] is False
    _assert_metadata_only(manifest["assets"])


def test_attach_asset_summary_updates_article_evidence_bundle_subtree(article_assets_contract) -> None:
    evidence_bundle = {
        "schema_version": "m024-article-evidence-bundle.v1",
        "paper_id": PAPER_ID,
        "source_refs": _source_refs(),
        "summary": {"source_count": 1},
        "subtrees": {"assets": {"status": "not_attempted"}},
        "diagnostics": [],
    }
    manifest = article_assets_contract.build_article_asset_manifest(_manifest_input())

    attached = article_assets_contract.attach_article_assets_summary(
        evidence_bundle,
        manifest,
        manifest_path="artifacts/article-assets.json",
        manifest_sha256="c" * 64,
    )

    assert attached["subtrees"]["assets"] == {
        "status": "review_only_not_import_eligible",
        "manifest_path": "artifacts/article-assets.json",
        "manifest_sha256": "c" * 64,
        "manifest_schema_version": article_assets_contract.ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION,
        "asset_count": 3,
        "asset_counts_by_type": {"equation_image": 1, "figure": 1, "table": 1},
        "preservation_state_counts": {"placeholder_only": 3},
        "interpretation_status_counts": {"not_interpreted": 3},
        "blocker_count": 0,
        "import_ineligible_count": 3,
        "hash_coverage_rate": 1.0,
        "page_index_anchor_coverage_rate": 1.0,
        "source_span_coverage_rate": 1.0,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }
    assert attached["summary"]["asset_count"] == 3
    assert attached["summary"]["asset_blocker_count"] == 0
    assert attached["summary"]["asset_import_ineligible_count"] == 3
    _assert_metadata_only(attached)


@pytest.mark.parametrize(
    ("mutate", "expected_code", "json_path", "object_id"),
    [
        (
            lambda payload: payload["asset_placeholders"][0].pop("source_file_id"),
            "missing_source_ref",
            "/asset_placeholders/0/source_file_id",
            "figure:1",
        ),
        (
            lambda payload: payload["asset_placeholders"][1].__setitem__("page_index_anchor_id", "missing-anchor"),
            "missing_page_index_ref",
            "/asset_placeholders/1/page_index_anchor_id",
            "table:1",
        ),
        (
            lambda payload: payload["source_refs"][0].__setitem__("sha256", "not-a-sha256"),
            "malformed_sha256",
            "/source_refs/0/sha256",
            SOURCE_ID,
        ),
        (
            lambda payload: payload["asset_placeholders"][2].__setitem__("interpretation_status", "interpreted_as_latex"),
            "invalid_interpretation_status",
            "/asset_placeholders/2/interpretation_status",
            "equation:1",
        ),
        (
            lambda payload: payload["asset_placeholders"].append(deepcopy(payload["asset_placeholders"][0])),
            "duplicate_asset_id",
            "/asset_placeholders/3/source_asset_ref",
            "figure:1",
        ),
        (
            lambda payload: payload["asset_placeholders"][0].__setitem__("trusted_kg_import_allowed", True),
            "unsafe_trusted_import_flag",
            "/asset_placeholders/0/trusted_kg_import_allowed",
            "figure:1",
        ),
    ],
)
def test_invalid_manifest_inputs_return_redacted_diagnostics_not_exceptions(
    article_assets_contract,
    mutate,
    expected_code: str,
    json_path: str,
    object_id: str,
) -> None:
    payload = _manifest_input()
    mutate(payload)

    manifest = article_assets_contract.build_article_asset_manifest(payload)

    assert expected_code in _diagnostic_codes(manifest)
    diagnostic = _diagnostic_by_code(manifest, expected_code)
    assert diagnostic["json_path"] == json_path
    assert diagnostic["object_id"] == object_id
    assert diagnostic["severity"] in {"repair_required", "error"}
    assert diagnostic["blocks_import"] is True
    assert manifest["summary"]["blocker_count"] >= 1
    assert manifest["subtree"]["status"] == "blocked"
    assert manifest["import_eligible_count"] == 0
    assert manifest["promoted_to_fact_count"] == 0
    _assert_metadata_only(manifest)


@pytest.mark.parametrize("forbidden_key", FORBIDDEN_PAYLOAD_KEYS)
def test_forbidden_payload_keys_are_diagnosed_without_echoing_values(article_assets_contract, forbidden_key: str) -> None:
    payload = _manifest_input()
    payload["asset_placeholders"][0][forbidden_key] = FORBIDDEN_SENTINEL

    manifest = article_assets_contract.build_article_asset_manifest(payload)

    assert "forbidden_payload_key" in _diagnostic_codes(manifest)
    diagnostic = _diagnostic_by_code(manifest, "forbidden_payload_key")
    assert diagnostic["json_path"] == f"/asset_placeholders/0/{forbidden_key}"
    assert diagnostic["object_id"] == "figure:1"
    assert FORBIDDEN_SENTINEL not in json.dumps(manifest["diagnostics"], sort_keys=True)
    assert manifest["summary"]["diagnostic_count_by_code"]["forbidden_payload_key"] >= 1
    assert manifest["subtree"]["status"] == "blocked"
    _assert_metadata_only(manifest)


def test_unsafe_graph_import_and_readiness_flags_fail_closed(article_assets_contract) -> None:
    payload = _manifest_input()
    payload["trusted_kg_import_allowed"] = True
    payload["production_import_attempted"] = True
    payload["ladybugdb_written"] = True
    payload["asset_placeholders"][0]["import_eligible"] = True
    payload["asset_placeholders"][1]["promoted_to_fact"] = True
    payload["asset_placeholders"][2]["readiness_status"] = "ready_for_import"

    manifest = article_assets_contract.build_article_asset_manifest(payload)

    assert {
        "unsafe_trusted_import_flag",
        "unsafe_production_import_flag",
        "unsafe_ladybugdb_written_flag",
        "unsafe_import_eligible_flag",
        "unsafe_promoted_to_fact_flag",
        "unsafe_readiness_status",
    } <= _diagnostic_codes(manifest)
    assert manifest["subtree"]["status"] == "blocked"
    assert manifest["subtree"]["trusted_kg_import_allowed"] is False
    assert manifest["subtree"]["ladybugdb_written"] is False
    assert manifest["subtree"]["production_import_attempted"] is False
    assert manifest["import_eligible_count"] == 0
    assert manifest["promoted_to_fact_count"] == 0
    _assert_metadata_only(manifest)

def test_article_assets_old_module_is_archived_with_canonical_breadcrumb(article_assets_contract) -> None:
    top_level_archive_path = Path("archive/package-layout-shims/wave-01/src/arxiv_archive/article_assets.py")
    package_archive_path = Path("archive/package-rename-waves/wave-01/src/arxiv_archive/artifacts/assets.py")
    canonical_path = Path("src/research_graph/papers/assets.py")

    assert top_level_archive_path.exists()
    assert package_archive_path.exists()
    assert not Path("src/arxiv_archive/article_assets.py").exists()
    assert not Path("src/arxiv_archive/artifacts/assets.py").exists()
    assert "Formerly: src/arxiv_archive/artifacts/assets.py" in canonical_path.read_text(encoding="utf-8")
    assert article_assets_contract.ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION == "m024-article-assets.v1"
