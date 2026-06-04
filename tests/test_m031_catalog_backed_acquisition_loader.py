from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from build_m031_catalog_backed_replay_selection import build_selection, main  # noqa: E402

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "build_m031_catalog_backed_replay_selection.py"
REAL_SOURCE_SELECTION = Path("data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json")
REAL_CATALOG = Path("data/article_catalog/catalog.json")
REAL_INDEX = Path("data/article_catalog/index.json")


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _safe_flags() -> dict[str, bool]:
    return {
        "source_acquired_now": False,
        "parser_ready_claimed": False,
        "chunk_ready_claimed": False,
        "graph_ready_claimed": False,
    }


def _fixture_tree(tmp_path: Path) -> tuple[Path, Path, Path, Path]:
    catalog_root = tmp_path / "article_catalog"
    catalog_path = catalog_root / "catalog.json"
    index_path = catalog_root / "index.json"
    article_path = catalog_root / "article_catalog" / "arxiv" / "mixed-source" / "2605.29548" / "article.json"
    selection_path = tmp_path / "selection.json"
    _write_json(
        catalog_path,
        {
            "schema_version": "article-catalog.v00.01",
            "safety_flags": {
                "graph_import_allowed": False,
                "production_ladybugdb_write_allowed": False,
                "trusted_kg_import_allowed": False,
                "production_import_attempted": False,
                "ladybugdb_written": False,
                "parser_ready_claimed": False,
                "chunk_ready_claimed": False,
                "kg_readiness_claimed": False,
            },
        },
    )
    _write_json(
        index_path,
        {
            "schema_version": "article-catalog-index.v00.01",
            "articles": [
                {
                    "article_ref": "arxiv/mixed-source/2605.29548",
                    "article_key": "2605.29548",
                    "source_code": "arxiv",
                    "coarse_topic_code": "mixed-source",
                    "canonical_url": "https://arxiv.org/abs/2605.29548",
                    "primary_source_role": "arxiv_abs_page",
                    "article_path": "article_catalog/arxiv/mixed-source/2605.29548/article.json",
                    "title": "Why Larger Models Learn More",
                }
            ],
        },
    )
    _write_json(
        article_path,
        {
            "schema_version": "article.v00.01",
            "article_key": "2605.29548",
            "catalog_path": "arxiv/mixed-source/2605.29548",
            "source_code": "arxiv",
            "coarse_topic_code": "mixed-source",
            "identity": {
                "title": "Why Larger Models Learn More",
                "canonical_url": "https://arxiv.org/abs/2605.29548",
                "normalized_identity": "arxiv:2605.29548",
            },
            "source_variants": [
                {
                    "variant_id": "2605.29548:source:arxiv-abs",
                    "source_role": "arxiv_abs_page",
                    "source_format": "html_metadata",
                    "source_origin": "provider_landing_page",
                    "is_primary": True,
                    "is_content_bearing": False,
                    "is_metadata_only": True,
                    "path": None,
                    "url": "https://arxiv.org/abs/2605.29548",
                    "media_type": "text/html",
                    "capture_status": "not_captured",
                    "loader_outcome": "not_loaded_metadata_only",
                    "requires_conversion": False,
                    "raw_text_embedded": False,
                    "raw_binary_embedded": False,
                    "network_fetch_attempted": False,
                    "parser_readiness_claimed": False,
                    "chunk_readiness_claimed": False,
                    "graph_readiness_claimed": False,
                }
            ],
            "safety_flags": {
                "metadata_manifests_embed_raw_text": False,
                "metadata_manifests_embed_raw_binary": False,
                "graph_import_allowed": False,
                "production_ladybugdb_write_allowed": False,
                "trusted_kg_import_allowed": False,
                "production_import_attempted": False,
                "ladybugdb_written": False,
                "parser_ready_claimed": False,
                "chunk_ready_claimed": False,
                "kg_readiness_claimed": False,
            },
        },
    )
    _write_json(
        selection_path,
        {
            "schema_version": "article-corpus-selection.v00.02",
            "selection_id": "m029-pipeline-architecture-audit-v1",
            "refs": [
                {
                    "ref_id": "m029-ref-003",
                    "url": "https://arxiv.org/abs/2605.29548",
                    "normalized_identity": "arxiv:2605.29548",
                    "source_kind": "arxiv_abs_url",
                    "catalog_status": "already_cataloged",
                    "known_title": "Why Larger Models Learn More",
                    "known_pdf_url": "https://arxiv.org/pdf/2605.29548",
                    "unsafe_claims": _safe_flags(),
                },
                {
                    "ref_id": "m029-ref-004",
                    "url": "https://arxiv.org/abs/2605.26099",
                    "normalized_identity": "arxiv:2605.26099",
                    "source_kind": "arxiv_abs_url",
                    "catalog_status": "typed_catalog_blocker",
                    "typed_blocker": {
                        "code": "catalog_placeholder_pruned_no_article_record",
                        "status": "blocked_until_metadata_record_registered",
                        "evidence": "index has no article.json-backed row",
                    },
                    "unsafe_claims": _safe_flags(),
                },
            ],
            "safety_flags": {
                "source_acquisition_completed": False,
                "raw_article_text_embedded": False,
                "binary_payload_embedded": False,
                "parser_ready_claimed": False,
                "chunk_ready_claimed": False,
                "kg_readiness_claimed": False,
                "graph_write_attempted": False,
                "production_persistence_attempted": False,
            },
        },
    )
    return selection_path, catalog_path, index_path, tmp_path / "selection-out.json"


def test_build_m031_real_selection_contract_writes_expected_counts(tmp_path: Path) -> None:
    output = tmp_path / "selection.json"

    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--source-selection",
            str(REAL_SOURCE_SELECTION),
            "--catalog",
            str(REAL_CATALOG),
            "--index",
            str(REAL_INDEX),
            "--output",
            str(output),
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0, result.stderr
    payload = json.loads(output.read_text(encoding="utf-8"))
    assert payload["counts"]["requested_ref_count"] == 4
    assert payload["counts"]["catalog_backed_count"] == 3
    assert payload["counts"]["typed_catalog_blocker_count"] == 1
    assert payload["counts"]["silent_missing_count"] == 0
    assert {row["identity"] for row in payload["articles"]} == {
        "arxiv:2507.19457",
        "stanford:cs224n:gradient-notes",
        "arxiv:2605.29548",
    }
    assert payload["catalog_blockers"][0]["identity"] == "arxiv:2605.26099"
    assert payload["catalog_blockers"][0]["blocker_code"] == "catalog_placeholder_pruned_no_article_record"
    for article in payload["articles"]:
        assert article["article_ref"]
        assert article["article_path"].endswith("article.json")
        assert article["source_variants"]
    assert payload["safety_flags"]["graph_import_allowed"] is False
    assert payload["safety_flags"]["production_ladybugdb_write_allowed"] is False
    assert payload["safety_flags"]["parser_ready_claimed"] is False
    assert payload["safety_flags"]["chunk_ready_claimed"] is False
    serialized = json.dumps(payload)
    assert "<html" not in serialized.lower()
    assert "%PDF-" not in serialized
    assert "base64," not in serialized.lower()


def test_build_m031_preserves_null_source_paths_as_metadata_blocker_inputs(tmp_path: Path) -> None:
    selection, catalog, index, output = _fixture_tree(tmp_path)

    assert main(["--source-selection", str(selection), "--catalog", str(catalog), "--index", str(index), "--output", str(output)]) == 0

    payload = json.loads(output.read_text(encoding="utf-8"))
    variants = payload["articles"][0]["source_variants"]
    assert variants[0]["path"] is None
    assert variants[0]["source_path_status"] == "path_absent"
    assert payload["catalog_blockers"][0]["article_ref"] is None


def test_build_m031_rejects_empty_refs(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    payload["refs"] = []
    _write_json(selection, payload)

    assert main(["--source-selection", str(selection), "--catalog", str(catalog), "--index", str(index), "--output", str(tmp_path / "out.json")]) == 2


def test_build_m031_rejects_duplicate_normalized_identities(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    payload["refs"].append(dict(payload["refs"][0], ref_id="duplicate"))
    _write_json(selection, payload)

    assert main(["--source-selection", str(selection), "--catalog", str(catalog), "--index", str(index), "--output", str(tmp_path / "out.json")]) == 2


def test_build_m031_rejects_missing_index_rows_for_cataloged_refs(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    index_payload = json.loads(index.read_text(encoding="utf-8"))
    index_payload["articles"] = []
    _write_json(index, index_payload)

    assert main(["--source-selection", str(selection), "--catalog", str(catalog), "--index", str(index), "--output", str(tmp_path / "out.json")]) == 2


def test_build_m031_rejects_path_traversal_in_index_article_path(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    index_payload = json.loads(index.read_text(encoding="utf-8"))
    index_payload["articles"][0]["article_path"] = "../escape/article.json"
    _write_json(index, index_payload)

    assert main(["--source-selection", str(selection), "--catalog", str(catalog), "--index", str(index), "--output", str(tmp_path / "out.json")]) == 2


def test_build_m031_rejects_url_as_index_article_path(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    index_payload = json.loads(index.read_text(encoding="utf-8"))
    index_payload["articles"][0]["article_path"] = "https://example.test/article.json"
    _write_json(index, index_payload)

    assert main(["--source-selection", str(selection), "--catalog", str(catalog), "--index", str(index), "--output", str(tmp_path / "out.json")]) == 2


def test_build_m031_rejects_true_unsafe_safety_flags(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    payload["refs"][0]["unsafe_claims"]["graph_ready_claimed"] = True
    _write_json(selection, payload)

    assert main(["--source-selection", str(selection), "--catalog", str(catalog), "--index", str(index), "--output", str(tmp_path / "out.json")]) == 2


def test_build_m031_rejects_unsafe_source_variant_paths(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    article_path = tmp_path / "article_catalog" / "article_catalog" / "arxiv" / "mixed-source" / "2605.29548" / "article.json"
    article_payload = json.loads(article_path.read_text(encoding="utf-8"))
    article_payload["source_variants"][0]["path"] = "../escape.html"
    _write_json(article_path, article_payload)

    assert main(["--source-selection", str(selection), "--catalog", str(catalog), "--index", str(index), "--output", str(tmp_path / "out.json")]) == 2


def test_build_m031_exposes_typed_blocker_diagnostics(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)

    payload = build_selection(selection, catalog, index)

    blocker_diagnostics = [row for row in payload["diagnostics"] if row["identity"] == "arxiv:2605.26099"]
    assert blocker_diagnostics == [
        {
            "code": "catalog_placeholder_pruned_no_article_record",
            "identity": "arxiv:2605.26099",
            "article_ref": None,
            "article_path": None,
            "source_role": "arxiv_abs_url",
            "safe_local_paths": [],
            "fail_closed_safety_flags": payload["safety_flags"],
        }
    ]
