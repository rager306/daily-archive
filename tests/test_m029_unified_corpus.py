from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

REGISTER_SCRIPT = Path(__file__).parents[1] / "scripts" / "register_m029_unified_corpus.py"
VERIFY_SCRIPT = Path(__file__).parents[1] / "scripts" / "verify_m029_unified_corpus_selection.py"


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _selection(selection_id: str, rows: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "schema_version": "article-corpus-selection.v00.01",
        "selection_id": selection_id,
        "articles": rows,
        "safety_flags": {
            "graph_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }


def _catalog(root: Path) -> None:
    entries = [
        {
            "article_ref": "arxiv/cs-ai/2512.24601",
            "article_key": "2512.24601",
            "source_code": "arxiv",
            "coarse_topic_code": "cs-ai",
            "canonical_url": "https://arxiv.org/abs/2512.24601",
            "primary_source_role": "arxiv_abs_page",
            "content_fallback_roles": ["arxiv_pdf"],
            "metadata_roles": ["arxiv_abs_page"],
            "article_path": "article_catalog/arxiv/cs-ai/2512.24601/article.json",
            "title": "Fixture 2512.24601",
        },
        {
            "article_ref": "arxiv/mixed-source/2605.20897",
            "article_key": "2605.20897",
            "source_code": "arxiv",
            "coarse_topic_code": "mixed-source",
            "canonical_url": "https://arxiv.org/abs/2605.20897",
            "primary_source_role": "arxiv_abs_page",
            "content_fallback_roles": ["arxiv_pdf"],
            "metadata_roles": ["arxiv_abs_page"],
            "article_path": "article_catalog/arxiv/mixed-source/2605.20897/article.json",
            "title": "Fixture 2605.20897",
        },
    ]
    _write_json(
        root / "catalog.json",
        {
            "schema_version": "article-catalog.v00.01",
            "article_schema_version": "article.v00.01",
            "index": {"path": "index.json"},
        },
    )
    for entry in entries:
        _write_json(
            # pyrefly: ignore [unsupported-operation]
            root / "article_catalog" / entry["article_ref"] / "article.json",  # ty:ignore[unsupported-operator]
            {
                "schema_version": "article.v00.01",
                "article_key": entry["article_key"],
                "catalog_path": entry["article_ref"],
                "source_code": entry["source_code"],
                "coarse_topic_code": entry["coarse_topic_code"],
                "identity": {"title": entry["title"], "canonical_url": entry["canonical_url"]},
                "source_strategy": {"primary_source_variant_id": f"{entry['article_key']}:primary"},
                "source_variants": [
                    {
                        "variant_id": f"{entry['article_key']}:primary",
                        "source_role": entry["primary_source_role"],
                        "is_primary": True,
                        "is_content_bearing": entry["primary_source_role"] != "arxiv_abs_page",
                        "is_metadata_only": entry["primary_source_role"] == "arxiv_abs_page",
                    }
                ],
            },
        )
    _write_json(
        root / "index.json",
        {
            "schema_version": "article-catalog-index.v00.01",
            "articles": entries,
            "indexes": {
                "by_canonical_url": {
                    entry["canonical_url"]: entry["article_ref"] for entry in entries
                },
                "by_article_key": {entry["article_key"]: entry["article_ref"] for entry in entries},
            },
        },
    )


def _fixture_inputs(tmp_path: Path) -> tuple[Path, Path, Path, Path, Path]:
    m025 = tmp_path / "m025" / "selection.json"
    m027 = tmp_path / "m027" / "selection.json"
    m028 = tmp_path / "M028-ROADMAP.md"
    catalog_root = tmp_path / "catalog"
    output_dir = tmp_path / "out"
    _catalog(catalog_root)
    _write_json(
        m025,
        _selection(
            "m025",
            [
                {
                    "article_ref": "arxiv/cs-ai/2512.24601",
                    "source_code": "arxiv",
                    "seed_url": "https://arxiv.org/abs/2512.24601",
                },
                {"source_code": "arxiv", "seed_url": "https://arxiv.org/html/2605.28617v1"},
                {"source_code": "arxiv", "seed_url": "https://arxiv.org/html/2605.26525v1"},
                {"source_code": "arxiv", "seed_url": "https://arxiv.org/abs/2507.19457"},
                {
                    "source_code": "company_blog",
                    "seed_url": "https://pageindex.ai/blog/pageindex-intro",
                },
            ],
        ),
    )
    _write_json(
        m027,
        _selection(
            "m027",
            [
                {
                    "article_ref": "arxiv/mixed-source/2605.20897",
                    "source_code": "arxiv",
                    "seed_url": "https://arxiv.org/pdf/2605.20897",
                    "canonical_url": "https://arxiv.org/abs/2605.20897",
                },
                {"source_code": "arxiv", "seed_url": "https://arxiv.org/abs/2605.21401"},
                {
                    "source_code": "nature",
                    "seed_url": "https://www.nature.com/articles/s44387-025-00019-5",
                },
                {"source_code": "arxiv", "seed_url": "https://arxiv.org/abs/2605.25522"},
                {"source_code": "arxiv", "seed_url": "https://arxiv.org/abs/2603.04448"},
                {"source_code": "arxiv", "seed_url": "https://arxiv.org/abs/2604.18478"},
            ],
        ),
    )
    m028.write_text(
        """
    Existing M028 refs:
      - https://arxiv.org/abs/2605.20897
    Newly accepted expansion refs:
      - https://arxiv.org/abs/2605.23904
      - https://arxiv.org/abs/2605.22502
      - https://arxiv.org/abs/2605.28655
      - https://arxiv.org/abs/2605.26099
      - https://arxiv.org/abs/2605.22166
      - https://arxiv.org/abs/2605.22681
      - https://arxiv.org/abs/2605.26302
    source-kind classification:
""",
        encoding="utf-8",
    )
    return m025, m027, m028, catalog_root, output_dir


def test_register_m029_writes_18_article_selection_with_duplicate_provenance(
    tmp_path: Path,
) -> None:
    m025, m027, m028, catalog_root, output_dir = _fixture_inputs(tmp_path)

    result = subprocess.run(
        [
            sys.executable,
            str(REGISTER_SCRIPT),
            "--write",
            "--m025-selection",
            str(m025),
            "--m027-selection",
            str(m027),
            "--m028-roadmap",
            str(m028),
            "--catalog-root",
            str(catalog_root),
            "--output-dir",
            str(output_dir),
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0, result.stderr
    selection = json.loads((output_dir / "selection.json").read_text(encoding="utf-8"))
    summary = json.loads((output_dir / "selection-summary.json").read_text(encoding="utf-8"))
    assert len(selection["articles"]) == 18
    assert summary["unique_article_count"] == 18
    assert summary["duplicate_urls"]["https://arxiv.org/abs/2605.20897"] == 2
    assert summary["index_resolution"] == {"resolved": 2, "unresolved": 16}
    assert selection["network_policy"]["registration_command_fetches_network"] is False


def test_verify_m029_rejects_missing_expected_duplicate_url(tmp_path: Path) -> None:
    m025, m027, m028, catalog_root, output_dir = _fixture_inputs(tmp_path)
    subprocess.run(
        [
            sys.executable,
            str(REGISTER_SCRIPT),
            "--write",
            "--m025-selection",
            str(m025),
            "--m027-selection",
            str(m027),
            "--m028-roadmap",
            str(m028),
            "--catalog-root",
            str(catalog_root),
            "--output-dir",
            str(output_dir),
        ],
        check=True,
        text=True,
        capture_output=True,
    )

    result = subprocess.run(
        [
            sys.executable,
            str(VERIFY_SCRIPT),
            "--selection",
            str(output_dir / "selection.json"),
            "--catalog",
            str(catalog_root / "catalog.json"),
            "--expect-unique-article-count",
            "18",
            "--expect-duplicate-url",
            "https://arxiv.org/abs/does-not-exist",
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 1
    assert "expected_duplicate_url_missing" in result.stderr


def test_register_m029_rejects_malformed_selection(tmp_path: Path) -> None:
    m025, m027, m028, catalog_root, output_dir = _fixture_inputs(tmp_path)
    m025.write_text("{not-json", encoding="utf-8")

    result = subprocess.run(
        [
            sys.executable,
            str(REGISTER_SCRIPT),
            "--write",
            "--m025-selection",
            str(m025),
            "--m027-selection",
            str(m027),
            "--m028-roadmap",
            str(m028),
            "--catalog-root",
            str(catalog_root),
            "--output-dir",
            str(output_dir),
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 1
    assert "m029_unified_registry_failed" in result.stderr
    assert not (output_dir / "selection.json").exists()


def test_verify_m029_rebuilds_index_with_selection_only_lookup_entries(tmp_path: Path) -> None:
    m025, m027, m028, catalog_root, output_dir = _fixture_inputs(tmp_path)
    subprocess.run(
        [
            sys.executable,
            str(REGISTER_SCRIPT),
            "--write",
            "--m025-selection",
            str(m025),
            "--m027-selection",
            str(m027),
            "--m028-roadmap",
            str(m028),
            "--catalog-root",
            str(catalog_root),
            "--output-dir",
            str(output_dir),
        ],
        check=True,
        text=True,
        capture_output=True,
    )

    index_path = catalog_root / "index.json"
    report_path = catalog_root / "index-rebuild-report.json"
    diagnostics_path = catalog_root / "index-rebuild-diagnostics.jsonl"
    result = subprocess.run(
        [
            sys.executable,
            str(VERIFY_SCRIPT),
            "--selection",
            str(output_dir / "selection.json"),
            "--catalog",
            str(catalog_root / "catalog.json"),
            "--rebuild-index",
            "--write-index",
            str(index_path),
            "--write-index-report",
            str(report_path),
            "--write-diagnostics",
            str(diagnostics_path),
            "--check-index-titles",
            "--check-index-idempotent",
            "--check-safe-traversal",
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0, result.stderr
    index = json.loads(index_path.read_text(encoding="utf-8"))
    report = json.loads(report_path.read_text(encoding="utf-8"))
    assert report["records_scanned"] == 2
    assert report["selection_entries_considered"] == 18
    assert report["selection_stub_entries_added"] == 16
    assert report["entries_emitted"] == 18
    assert report["idempotent"] is True
    assert (
        index["indexes"]["by_canonical_url"]["https://arxiv.org/abs/2605.23904"]
        == "arxiv/mixed-source/2605.23904"
    )
    placeholder = next(
        entry
        for entry in index["articles"]
        if entry["article_ref"] == "arxiv/mixed-source/2605.23904"
    )
    assert placeholder["catalog_record_present"] is False
    assert placeholder["title_status"] == "unresolved_catalog_placeholder"
    assert not diagnostics_path.read_text(encoding="utf-8")


def test_verify_m029_rejects_unsafe_selection_article_ref_during_rebuild(tmp_path: Path) -> None:
    m025, m027, m028, catalog_root, output_dir = _fixture_inputs(tmp_path)
    subprocess.run(
        [
            sys.executable,
            str(REGISTER_SCRIPT),
            "--write",
            "--m025-selection",
            str(m025),
            "--m027-selection",
            str(m027),
            "--m028-roadmap",
            str(m028),
            "--catalog-root",
            str(catalog_root),
            "--output-dir",
            str(output_dir),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    selection_path = output_dir / "selection.json"
    selection = json.loads(selection_path.read_text(encoding="utf-8"))
    selection["articles"][-1]["article_ref"] = "../escape"
    selection_path.write_text(json.dumps(selection), encoding="utf-8")

    result = subprocess.run(
        [
            sys.executable,
            str(VERIFY_SCRIPT),
            "--selection",
            str(selection_path),
            "--catalog",
            str(catalog_root / "catalog.json"),
            "--rebuild-index",
            "--check-safe-traversal",
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 1
    assert "selection_stub_entry_failed" in result.stderr


def test_verify_m029_validate_only_writes_selection_contract_artifacts(tmp_path: Path) -> None:
    m025, m027, m028, catalog_root, output_dir = _fixture_inputs(tmp_path)
    subprocess.run(
        [
            sys.executable,
            str(REGISTER_SCRIPT),
            "--write",
            "--m025-selection",
            str(m025),
            "--m027-selection",
            str(m027),
            "--m028-roadmap",
            str(m028),
            "--catalog-root",
            str(catalog_root),
            "--output-dir",
            str(output_dir),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    subprocess.run(
        [
            sys.executable,
            str(VERIFY_SCRIPT),
            "--selection",
            str(output_dir / "selection.json"),
            "--catalog",
            str(catalog_root / "catalog.json"),
            "--rebuild-index",
            "--write-index",
            str(catalog_root / "index.json"),
        ],
        check=True,
        text=True,
        capture_output=True,
    )

    summary_path = output_dir / "selection-summary.json"
    diagnostics_path = output_dir / "selection-diagnostics.jsonl"
    report_path = output_dir / "selection-report.md"
    result = subprocess.run(
        [
            sys.executable,
            str(VERIFY_SCRIPT),
            "--selection",
            str(output_dir / "selection.json"),
            "--catalog",
            str(catalog_root / "catalog.json"),
            "--index",
            str(catalog_root / "index.json"),
            "--validate-only",
            "--require-index",
            "--check-index-titles",
            "--check-safe-traversal",
            "--check-duplicate-lookups",
            "--write-summary",
            str(summary_path),
            "--write-diagnostics",
            str(diagnostics_path),
            "--write-report",
            str(report_path),
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0, result.stderr
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["validation_status"] == "passed"
    assert summary["unique_article_count"] == 18
    assert summary["index_resolution"] == {"resolved": 2, "unresolved": 16}
    assert summary["index_contract"]["selected_url_lookup_count"] == 18
    assert summary["index_contract"]["selected_article_key_lookup_count"] == 18
    assert summary["index_contract"]["title_bearing_catalog_rows"] == 2
    assert summary["index_contract"]["placeholder_title_count"] == 0
    assert not diagnostics_path.read_text(encoding="utf-8")
    assert "# M029 Unified Corpus Selection Report" in report_path.read_text(encoding="utf-8")


def test_verify_m029_validate_only_rejects_missing_required_index_lookup(tmp_path: Path) -> None:
    m025, m027, m028, catalog_root, output_dir = _fixture_inputs(tmp_path)
    subprocess.run(
        [
            sys.executable,
            str(REGISTER_SCRIPT),
            "--write",
            "--m025-selection",
            str(m025),
            "--m027-selection",
            str(m027),
            "--m028-roadmap",
            str(m028),
            "--catalog-root",
            str(catalog_root),
            "--output-dir",
            str(output_dir),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    subprocess.run(
        [
            sys.executable,
            str(VERIFY_SCRIPT),
            "--selection",
            str(output_dir / "selection.json"),
            "--catalog",
            str(catalog_root / "catalog.json"),
            "--rebuild-index",
            "--write-index",
            str(catalog_root / "index.json"),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    index_path = catalog_root / "index.json"
    index = json.loads(index_path.read_text(encoding="utf-8"))
    del index["indexes"]["by_canonical_url"]["https://arxiv.org/abs/2605.23904"]
    index_path.write_text(json.dumps(index), encoding="utf-8")

    result = subprocess.run(
        [
            sys.executable,
            str(VERIFY_SCRIPT),
            "--selection",
            str(output_dir / "selection.json"),
            "--catalog",
            str(catalog_root / "catalog.json"),
            "--index",
            str(index_path),
            "--validate-only",
            "--require-index",
            "--check-duplicate-lookups",
        ],
        check=False,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 1
    assert "selected_url_missing_from_index" in result.stderr
