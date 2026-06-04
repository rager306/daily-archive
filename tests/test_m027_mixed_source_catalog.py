from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

SCRIPT = Path(__file__).parents[1] / "scripts" / "verify_article_catalog.py"
WRAPPER_SCRIPT = Path(__file__).parents[1] / "scripts" / "verify_m027_mixed_source_catalog.py"
REGISTER_SCRIPT = Path(__file__).parents[1] / "scripts" / "register_m027_mixed_source_corpus.py"
REGISTER_M029_SCRIPT = Path(__file__).parents[1] / "scripts" / "register_m029_missing_metadata_refs.py"
VERIFY_M030_SCRIPT = Path(__file__).parents[1] / "scripts" / "verify_m030_requested_ref_intake.py"
SELECTION_ID = "m027-mixed-source-corpus-v1"


def _article(
    *,
    article_ref: str,
    title: str,
    canonical_url: str,
    role: str,
    source_format: str,
) -> dict[str, Any]:
    source_code, coarse_topic_code, article_key = article_ref.split("/", 2)
    return {
        "schema_version": "article.v00.01",
        "catalog_path": article_ref,
        "source_code": source_code,
        "coarse_topic_code": coarse_topic_code,
        "article_key": article_key,
        "identity": {
            "title": title,
            "canonical_url": canonical_url,
        },
        "source_variants": [
            {
                "variant_id": role,
                "source_role": role,
                "source_format": source_format,
                "is_primary": True,
                "is_content_bearing": source_format != "html_metadata",
                "is_metadata_only": source_format == "html_metadata",
                "capture_status": "not_captured",
                "loader_outcome": "loaded_metadata_only",
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
            }
        ],
        "safety_flags": {
            "graph_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }


def _index_entry(article_ref: str, article: dict[str, Any]) -> dict[str, Any]:
    source_code, coarse_topic_code, article_key = article_ref.split("/", 2)
    variant = article["source_variants"][0]
    return {
        "article_ref": article_ref,
        "article_key": article_key,
        "source_code": source_code,
        "coarse_topic_code": coarse_topic_code,
        "canonical_url": article["identity"]["canonical_url"],
        "primary_source_role": variant["source_role"],
        "content_fallback_roles": [],
        "metadata_roles": [variant["source_role"]] if variant["is_metadata_only"] else [],
        "article_path": f"article_catalog/{article_ref}/article.json",
        "title": article["identity"]["title"],
    }


def _build_indexes(entries: list[dict[str, Any]]) -> dict[str, Any]:
    indexes: dict[str, Any] = {
        "by_article_key": {},
        "by_citation_key": {},
        "by_source_code": {},
        "by_coarse_topic_code": {},
        "by_canonical_url": {},
        "by_title": {},
    }
    for entry in entries:
        ref = entry["article_ref"]
        indexes["by_article_key"][entry["article_key"]] = ref
        indexes["by_canonical_url"][entry["canonical_url"]] = ref
        indexes["by_title"][entry["title"]] = ref
        indexes["by_source_code"].setdefault(entry["source_code"], []).append(ref)
        indexes["by_coarse_topic_code"].setdefault(entry["coarse_topic_code"], []).append(ref)
    for name in ("by_source_code", "by_coarse_topic_code"):
        indexes[name] = {key: sorted(value) for key, value in sorted(indexes[name].items())}
    for name in ("by_article_key", "by_citation_key", "by_canonical_url", "by_title"):
        indexes[name] = {key: indexes[name][key] for key in sorted(indexes[name])}
    return indexes


def _copy_m027_scaffold(tmp_path: Path) -> tuple[Path, Path, Path]:
    catalog_dir = tmp_path / "article_catalog"
    catalog_records_dir = catalog_dir / "article_catalog"
    corpus_dir = tmp_path / "article_corpora" / SELECTION_ID
    catalog_dir.mkdir(parents=True)
    corpus_dir.mkdir(parents=True)
    (catalog_dir / "schemas").mkdir()
    for schema_name in ("article-catalog-schema.v00.01.json", "article-schema.v00.01.json"):
        (catalog_dir / "schemas" / schema_name).write_text(json.dumps({"type": "object"}), encoding="utf-8")

    articles = {
        "arxiv/cs-ai/2401.00001": _article(
            article_ref="arxiv/cs-ai/2401.00001",
            title="Direct PDF Seed Normalizes to arXiv Abstract Identity",
            canonical_url="https://arxiv.org/abs/2401.00001",
            role="arxiv_pdf",
            source_format="pdf",
        ),
        "arxiv/cs-cl/2401.00002": _article(
            article_ref="arxiv/cs-cl/2401.00002",
            title="Abstract Page Seed Keeps arXiv Abstract Identity",
            canonical_url="https://arxiv.org/abs/2401.00002",
            role="arxiv_abs_page",
            source_format="html_metadata",
        ),
        "nature/biotech/nature-2024-agentic-biology": _article(
            article_ref="nature/biotech/nature-2024-agentic-biology",
            title="Nature Fixture Row Preserves Publisher Metadata",
            canonical_url="https://www.nature.com/articles/s41587-024-00000-0",
            role="publisher_landing_page",
            source_format="html_metadata",
        ),
    }
    for article_ref, article in articles.items():
        path = catalog_records_dir / article_ref / "article.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(article, indent=2), encoding="utf-8")

    entries = [_index_entry(ref, article) for ref, article in articles.items()]
    index = {
        "schema_version": "article-catalog-index.v00.01",
        "catalog_schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "index_id": "m027_mixed_source_fixture_index",
        "generated_from": "article_catalog/",
        "lookup_policy": {
            "cli_must_use_index": True,
            "full_tree_scan_allowed": False,
            "refresh_command_rebuilds_index": True,
        },
        "articles": entries,
        "indexes": _build_indexes(entries),
        "safety_flags": {
            "metadata_manifests_embed_raw_text": False,
            "metadata_manifests_embed_raw_binary": False,
            "graph_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }
    selection = {
        "schema_version": "article-corpus-selection.v00.01",
        "selection_id": SELECTION_ID,
        "catalog_schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "purpose": "M027 mixed-source metadata-only corpus fixture.",
        "selection_mode": "manual_url_seed",
        "network_policy": {
            "capture_phase_may_fetch": False,
            "test_phase_must_not_fetch": True,
            "pipeline_phase_reads_catalog_only": True,
        },
        "articles": [
            {
                "article_ref": "arxiv/cs-ai/2401.00001",
                "source_code": "arxiv",
                "title": "Direct PDF Seed Normalizes to arXiv Abstract Identity",
                "seed_url": "https://arxiv.org/pdf/2401.00001.pdf",
            },
            {
                "article_ref": "arxiv/cs-cl/2401.00002",
                "source_code": "arxiv",
                "title": "Abstract Page Seed Keeps arXiv Abstract Identity",
                "seed_url": "https://arxiv.org/abs/2401.00002",
            },
            {
                "article_ref": "nature/biotech/nature-2024-agentic-biology",
                "source_code": "nature",
                "title": "Nature Fixture Row Preserves Publisher Metadata",
                "seed_url": "https://www.nature.com/articles/s41587-024-00000-0",
            },
        ],
        "safety_flags": {
            "graph_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }
    catalog = {
        "schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "root": "data/article_catalog",
        "path_template": "{source_code}/{coarse_topic_code}/{article_key}",
        "index": {
            "schema_version": "article-catalog-index.v00.01",
            "path": "index.json",
            "cli_must_use_index": True,
            "full_tree_scan_allowed": False,
            "refresh_command_rebuilds_index": True,
            "lookup_keys": ["article_key", "citation_key", "canonical_url", "source_code", "coarse_topic_code", "title"],
        },
        "safety_flags": {
            "metadata_manifests_embed_raw_text": False,
            "metadata_manifests_embed_raw_binary": False,
            "graph_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }
    (catalog_dir / "catalog.json").write_text(json.dumps(catalog, indent=2), encoding="utf-8")
    (catalog_dir / "index.json").write_text(json.dumps(index, indent=2), encoding="utf-8")
    (corpus_dir / "selection.json").write_text(json.dumps(selection, indent=2), encoding="utf-8")
    return catalog_dir / "catalog.json", catalog_dir / "index.json", corpus_dir / "selection.json"


def _run_generic(catalog: Path, index: Path, selection: Path, *extra: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--catalog",
            str(catalog),
            "--index",
            str(index),
            "--selection",
            str(selection),
            "--validate-only",
            "--require-index",
            "--check-index-titles",
            "--check-safe-traversal",
            "--check-duplicate-lookups",
            "--require-selection-titles",
            *extra,
        ],
        capture_output=True,
        check=False,
        text=True,
    )


def test_m027_wrapper_emits_local_only_handoff_artifacts() -> None:
    result = subprocess.run(
        [sys.executable, str(WRAPPER_SCRIPT)],
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert "M027 mixed-source catalog validation passed" in result.stdout

    corpus_dir = Path(__file__).parents[1] / "data" / "article_corpora" / SELECTION_ID
    summary = json.loads((corpus_dir / "catalog-summary.json").read_text(encoding="utf-8"))
    diagnostics = [json.loads(line) for line in (corpus_dir / "catalog-diagnostics.jsonl").read_text(encoding="utf-8").splitlines() if line.strip()]
    report = (corpus_dir / "catalog-report.md").read_text(encoding="utf-8")

    assert summary["milestone_id"] == "M027-aakeky"
    assert summary["slice_id"] == "S01"
    assert summary["article_count"] == 6
    assert summary["local_only_validation"]["network_fetch_attempted"] is False
    assert summary["local_only_validation"]["index_lookup_only"] is True
    assert summary["out_of_scope"] == {
        "captured_sources_claimed": False,
        "conversion_claimed": False,
        "parser_readiness_claimed": False,
        "chunks_claimed": False,
        "production_import_claimed": False,
        "trusted_facts_claimed": False,
        "ladybugdb_write_claimed": False,
    }
    assert summary["provenance"]["command"] == ["uv", "run", "python", "scripts/verify_m027_mixed_source_catalog.py"]
    assert summary["provenance"]["exit_code"] == 0
    assert "data/article_corpora/m027-mixed-source-corpus-v1/catalog-report.md" in summary["provenance"]["output_hashes"]

    expected_urls = [
        "https://arxiv.org/pdf/2605.20897",
        "https://arxiv.org/abs/2605.21401",
        "https://www.nature.com/articles/s44387-025-00019-5",
        "https://arxiv.org/abs/2605.25522",
        "https://arxiv.org/abs/2603.04448",
        "https://arxiv.org/abs/2604.18478",
    ]
    assert [row["seed_url"] for row in summary["articles"]] == expected_urls
    assert all(row["article_ref"] in report for row in summary["articles"])
    assert all(url in report for url in expected_urls)
    assert "Validate-only network_fetch_attempted=false" in report
    assert "Out of scope: captured sources, conversion, parser readiness, chunks, production imports, trusted facts, and LadybugDB writes." in report
    assert "Expected load is six selected articles" in report
    assert "Expected load is five selected articles" not in report
    assert diagnostics
    assert all(row.get("selection_id") == SELECTION_ID for row in diagnostics)
    assert all(row.get("network_fetch_attempted") is False for row in diagnostics)
    assert all("json_path" in row and "failing_invariant" in row and "file_path" in row for row in diagnostics)


def test_generic_verifier_accepts_m027_mixed_source_selection_id_and_url_shapes(tmp_path: Path) -> None:
    catalog, index, selection = _copy_m027_scaffold(tmp_path)
    summary = selection.parent / "run-summary.json"
    diagnostics = selection.parent / "diagnostics.jsonl"
    report = selection.parent / "catalog-report.md"

    result = _run_generic(
        catalog,
        index,
        selection,
        "--write-summary",
        str(summary),
        "--write-diagnostics",
        str(diagnostics),
        "--write-report",
        str(report),
    )

    assert result.returncode == 0, result.stderr
    assert "article catalog validation passed" in result.stdout
    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["selection_id"] == SELECTION_ID
    assert payload["network"]["network_fetch_attempted_during_validation"] is False
    assert payload["index"]["full_tree_scan_attempted"] is False
    indexed_urls = {row["canonical_url"] for row in json.loads(index.read_text(encoding="utf-8"))["articles"]}
    assert "https://arxiv.org/abs/2401.00001" in indexed_urls
    assert "https://arxiv.org/abs/2401.00002" in indexed_urls
    assert "https://www.nature.com/articles/s41587-024-00000-0" in indexed_urls
    assert "Article Catalog Readiness Report" in report.read_text(encoding="utf-8")


def test_generic_verifier_rejects_duplicate_lookup_keys_before_readiness_outputs(tmp_path: Path) -> None:
    catalog, index, selection = _copy_m027_scaffold(tmp_path)
    payload = json.loads(index.read_text(encoding="utf-8"))
    payload["articles"][1]["title"] = payload["articles"][0]["title"]
    index.write_text(json.dumps(payload), encoding="utf-8")
    summary = selection.parent / "run-summary.json"

    result = _run_generic(catalog, index, selection, "--write-summary", str(summary))

    assert result.returncode == 1
    assert "duplicate lookup key title" in result.stderr
    assert not summary.exists()


def test_generic_verifier_rejects_selection_title_drift(tmp_path: Path) -> None:
    catalog, index, selection = _copy_m027_scaffold(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    payload["articles"][0]["title"] = "Drifted M027 Title"
    selection.write_text(json.dumps(payload), encoding="utf-8")

    result = _run_generic(catalog, index, selection)

    assert result.returncode == 1
    assert "title does not match index" in result.stderr


def test_generic_verifier_rejects_missing_selection_title(tmp_path: Path) -> None:
    catalog, index, selection = _copy_m027_scaffold(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    del payload["articles"][0]["title"]
    selection.write_text(json.dumps(payload), encoding="utf-8")

    result = _run_generic(catalog, index, selection)

    assert result.returncode == 1
    assert "title must be a non-empty string" in result.stderr


def test_generic_verifier_rejects_selection_not_in_index(tmp_path: Path) -> None:
    catalog, index, selection = _copy_m027_scaffold(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    payload["articles"][0]["article_ref"] = "arxiv/cs-ai/missing"
    selection.write_text(json.dumps(payload), encoding="utf-8")

    result = _run_generic(catalog, index, selection)

    assert result.returncode == 1
    assert "selection article_ref not present in index" in result.stderr


def test_generic_verifier_rejects_unsafe_article_path_traversal(tmp_path: Path) -> None:
    catalog, index, selection = _copy_m027_scaffold(tmp_path)
    payload = json.loads(index.read_text(encoding="utf-8"))
    payload["articles"][0]["article_path"] = "../outside/article.json"
    index.write_text(json.dumps(payload), encoding="utf-8")

    result = _run_generic(catalog, index, selection)

    assert result.returncode == 1
    assert "unsafe catalog-relative path" in result.stderr or "non-canonical" in result.stderr


def test_generic_verifier_rejects_malformed_selection_json_with_path(tmp_path: Path) -> None:
    catalog, index, selection = _copy_m027_scaffold(tmp_path)
    selection.write_text('{"schema_version": ', encoding="utf-8")

    result = _run_generic(catalog, index, selection)

    assert result.returncode == 1
    assert "malformed JSON" in result.stderr
    assert str(selection) in result.stderr


def test_generic_verifier_allows_shared_index_superset_when_requested(tmp_path: Path) -> None:
    catalog, index, selection = _copy_m027_scaffold(tmp_path)
    payload = json.loads(index.read_text(encoding="utf-8"))
    extra_ref = "arxiv/cs-ai/2401.09999"
    extra_article = _article(
        article_ref=extra_ref,
        title="Shared Index Extra Row Outside This Selection",
        canonical_url="https://arxiv.org/abs/2401.09999",
        role="arxiv_abs_page",
        source_format="html_metadata",
    )
    extra_path = index.parents[0] / "article_catalog" / extra_ref / "article.json"
    extra_path.parent.mkdir(parents=True, exist_ok=True)
    extra_path.write_text(json.dumps(extra_article, indent=2), encoding="utf-8")
    payload["articles"].append(_index_entry(extra_ref, extra_article))
    payload["indexes"] = _build_indexes(payload["articles"])
    index.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    strict_result = _run_generic(catalog, index, selection)
    assert strict_result.returncode == 1
    assert "index articles missing from selection" in strict_result.stderr

    superset_result = _run_generic(catalog, index, selection, "--allow-index-superset")
    assert superset_result.returncode == 0, superset_result.stderr


def test_registration_command_writes_six_rows_idempotently_and_preserves_existing_index(tmp_path: Path) -> None:
    catalog_root = tmp_path / "article_catalog"
    corpora_root = tmp_path / "article_corpora"
    catalog_root.mkdir(parents=True)
    preserved_ref = "arxiv/cs-ai/2512.24601"
    preserved_entry = {
        "article_ref": preserved_ref,
        "article_key": "2512.24601",
        "source_code": "arxiv",
        "coarse_topic_code": "cs-ai",
        "canonical_url": "https://arxiv.org/abs/2512.24601",
        "primary_source_role": "arxiv_html",
        "content_fallback_roles": ["arxiv_pdf"],
        "metadata_roles": ["arxiv_abs_page"],
        "article_path": f"article_catalog/{preserved_ref}/article.json",
        "title": "Recursive Language Models",
    }
    index_payload = {
        "schema_version": "article-catalog-index.v00.01",
        "catalog_schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "index_id": "temp_index",
        "generated_from": "article_catalog/",
        "lookup_policy": {"cli_must_use_index": True, "full_tree_scan_allowed": False, "refresh_command_rebuilds_index": True},
        "articles": [preserved_entry],
        "indexes": _build_indexes([preserved_entry]),
        "safety_flags": {"graph_import_allowed": False, "production_ladybugdb_write_allowed": False},
    }
    (catalog_root / "index.json").write_text(json.dumps(index_payload, indent=2), encoding="utf-8")

    command = [
        sys.executable,
        str(REGISTER_SCRIPT),
        "--write",
        "--catalog-root",
        str(catalog_root),
        "--corpora-root",
        str(corpora_root),
    ]
    first = subprocess.run(command, capture_output=True, check=False, text=True)
    assert first.returncode == 0, first.stderr
    before = (catalog_root / "index.json").read_text(encoding="utf-8")
    second = subprocess.run(command, capture_output=True, check=False, text=True)
    assert second.returncode == 0, second.stderr
    assert (catalog_root / "index.json").read_text(encoding="utf-8") == before

    index_after = json.loads(before)
    refs = {row["article_ref"] for row in index_after["articles"]}
    assert preserved_ref in refs
    assert len([ref for ref in refs if "/mixed-source/" in ref]) == 6
    selection = json.loads((corpora_root / SELECTION_ID / "selection.json").read_text(encoding="utf-8"))
    assert [row["seed_url"] for row in selection["articles"]] == [
        "https://arxiv.org/pdf/2605.20897",
        "https://arxiv.org/abs/2605.21401",
        "https://www.nature.com/articles/s44387-025-00019-5",
        "https://arxiv.org/abs/2605.25522",
        "https://arxiv.org/abs/2603.04448",
        "https://arxiv.org/abs/2604.18478",
    ]
    assert all(row["title"] for row in selection["articles"])
    assert "raw_text_payload" not in before
    assert "raw_binary_payload" not in before
    assert all(row.get("network_fetch_attempted") is False for row in selection["articles"])


def test_m029_registration_command_writes_two_metadata_only_refs_with_fail_closed_flags(tmp_path: Path) -> None:
    catalog_root = tmp_path / "article_catalog"
    catalog_root.mkdir(parents=True)
    preserved_ref = "arxiv/cs-ai/2512.24601"
    preserved_entry = {
        "article_ref": preserved_ref,
        "article_key": "2512.24601",
        "source_code": "arxiv",
        "coarse_topic_code": "cs-ai",
        "canonical_url": "https://arxiv.org/abs/2512.24601",
        "primary_source_role": "arxiv_html",
        "content_fallback_roles": ["arxiv_pdf"],
        "metadata_roles": ["arxiv_abs_page"],
        "article_path": f"article_catalog/{preserved_ref}/article.json",
        "title": "Recursive Language Models",
    }
    index_payload = {
        "schema_version": "article-catalog-index.v00.01",
        "catalog_schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "index_id": "temp_index",
        "generated_from": "article_catalog/",
        "lookup_policy": {"cli_must_use_index": True, "full_tree_scan_allowed": False, "refresh_command_rebuilds_index": True},
        "articles": [preserved_entry],
        "indexes": _build_indexes([preserved_entry]),
        "safety_flags": {"graph_import_allowed": False, "production_ladybugdb_write_allowed": False},
    }
    catalog_payload = {
        "schema_version": "article-catalog.v00.01",
        "article_schema_version": "article.v00.01",
        "root": "data/article_catalog",
        "sources": [
            {
                "source_code": "arxiv",
                "source_type": "preprint_server",
                "allowed_source_roles": ["arxiv_abs_page", "arxiv_pdf"],
            }
        ],
        "safety_flags": {"graph_import_allowed": False, "production_ladybugdb_write_allowed": False},
    }
    (catalog_root / "catalog.json").write_text(json.dumps(catalog_payload, indent=2), encoding="utf-8")
    (catalog_root / "index.json").write_text(json.dumps(index_payload, indent=2), encoding="utf-8")

    command = [sys.executable, str(REGISTER_M029_SCRIPT), "--write", "--catalog-root", str(catalog_root)]
    first = subprocess.run(command, capture_output=True, check=False, text=True)
    assert first.returncode == 0, first.stderr
    assert "duplicate_normalized_identity" not in first.stderr
    assert "unsafe_article_ref" not in first.stderr
    assert "missing_metadata" not in first.stderr
    assert "unsafe_readiness_or_persistence_flag" not in first.stderr
    before = (catalog_root / "index.json").read_text(encoding="utf-8")
    second = subprocess.run(command, capture_output=True, check=False, text=True)
    assert second.returncode == 0, second.stderr
    assert (catalog_root / "index.json").read_text(encoding="utf-8") == before

    index_after = json.loads(before)
    refs = {row["article_ref"]: row for row in index_after["articles"]}
    assert preserved_ref in refs
    assert "stanford/cs224n/gradient-notes" in refs
    assert "arxiv/mixed-source/2605.29548" in refs
    assert refs["stanford/cs224n/gradient-notes"]["normalized_identity"] == "stanford:cs224n:gradient-notes"
    assert refs["arxiv/mixed-source/2605.29548"]["normalized_identity"] == "arxiv:2605.29548"
    assert index_after["indexes"]["by_source_code"]["stanford"] == ["stanford/cs224n/gradient-notes"]

    catalog_after = json.loads((catalog_root / "catalog.json").read_text(encoding="utf-8"))
    assert any(source["source_code"] == "stanford" for source in catalog_after["sources"])

    serialized = json.dumps(
        {
            "catalog": catalog_after,
            "index": index_after,
            "articles": [
                json.loads((catalog_root / refs["stanford/cs224n/gradient-notes"]["article_path"]).read_text(encoding="utf-8")),
                json.loads((catalog_root / refs["arxiv/mixed-source/2605.29548"]["article_path"]).read_text(encoding="utf-8")),
            ],
        },
        sort_keys=True,
    )
    forbidden_snippets = [
        "trusted_kg_import_allowed\": true",
        "production_ladybugdb_write_allowed\": true",
        "raw_text_embedded\": true",
        "raw_binary_embedded\": true",
        "production_import_attempted\": true",
        "ladybugdb_written\": true",
        "network_fetch_attempted\": true",
        "parser_readiness_claimed\": true",
        "chunk_readiness_claimed\": true",
        "graph_readiness_claimed\": true",
    ]
    for snippet in forbidden_snippets:
        assert snippet not in serialized


def test_m030_requested_ref_intake_closeout_baseline_is_current() -> None:
    root = Path(__file__).parents[1]
    result = subprocess.run(
        [
            sys.executable,
            str(VERIFY_M030_SCRIPT),
            "--selection",
            str(root / "data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json"),
            "--report",
            str(root / "data/article_corpora/m029-pipeline-architecture-audit-v1/intake-report.md"),
            "--catalog-index",
            str(root / "data/article_catalog/index.json"),
            "--m028-selection",
            str(root / "data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json"),
            "--validate-only",
        ],
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert "4 refs, 3 cataloged, 1 typed catalog blocker" in result.stdout


def test_m030_requested_ref_intake_rejects_unsafe_claims(tmp_path: Path) -> None:
    root = Path(__file__).parents[1]
    selection = json.loads(
        (root / "data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json").read_text(encoding="utf-8")
    )
    selection["refs"][0]["unsafe_claims"]["graph_ready_claimed"] = True
    unsafe_selection = tmp_path / "selection.json"
    unsafe_selection.write_text(json.dumps(selection, indent=2), encoding="utf-8")

    result = subprocess.run(
        [
            sys.executable,
            str(VERIFY_M030_SCRIPT),
            "--selection",
            str(unsafe_selection),
            "--report",
            str(root / "data/article_corpora/m029-pipeline-architecture-audit-v1/intake-report.md"),
            "--catalog-index",
            str(root / "data/article_catalog/index.json"),
            "--m028-selection",
            str(root / "data/article_corpora/m028-universal-loader-runtime-smoke-v1/selection.json"),
            "--validate-only",
        ],
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 1
    assert "M030_INTAKE_UNSAFE_CLAIMS" in result.stderr
