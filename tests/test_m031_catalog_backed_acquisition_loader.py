from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

# pyrefly: ignore [missing-import]
from build_m031_catalog_backed_replay_selection import (  # noqa: E402  # ty:ignore[unresolved-import]
    build_selection,
    main,
)
from replay_m031_catalog_backed_acquisition import (  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
    build_summary as build_acquisition_summary,  # noqa: E402
)

# pyrefly: ignore [missing-import]
from replay_m031_catalog_backed_acquisition import (
    main as replay_main,  # noqa: E402  # ty:ignore[unresolved-import]
)
from replay_m031_catalog_backed_acquisition import (  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
    replay_selection,
    sha256_file,
)
from replay_m031_catalog_backed_loader_evidence import (  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
    LoaderEvidenceError,
    assert_fail_closed_flags,
    replay_loader_evidence,
)

# pyrefly: ignore [missing-import]
from replay_m031_catalog_backed_loader_evidence import (  # ty:ignore[unresolved-import]
    build_summary as build_loader_summary,
)

# pyrefly: ignore [missing-import]
from replay_m031_catalog_backed_loader_evidence import (  # ty:ignore[unresolved-import]
    main as loader_main,
)

# pyrefly: ignore [missing-import]
from verify_m031_catalog_backed_replay import (  # noqa: E402  # ty:ignore[unresolved-import]
    CloseoutError,
    verify_contract,
)

SCRIPT = (
    Path(__file__).resolve().parents[1]
    / "scripts"
    / "build_m031_catalog_backed_replay_selection.py"
)
REPLAY_SCRIPT = (
    Path(__file__).resolve().parents[1] / "scripts" / "replay_m031_catalog_backed_acquisition.py"
)
LOADER_SCRIPT = (
    Path(__file__).resolve().parents[1]
    / "scripts"
    / "replay_m031_catalog_backed_loader_evidence.py"
)
CLOSEOUT_SCRIPT = (
    Path(__file__).resolve().parents[1] / "scripts" / "verify_m031_catalog_backed_replay.py"
)
REAL_SOURCE_SELECTION = Path(
    "data/article_corpora/m029-pipeline-architecture-audit-v1/selection.json"
)
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
    article_path = (
        catalog_root / "article_catalog" / "arxiv" / "mixed-source" / "2605.29548" / "article.json"
    )
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
    assert (
        payload["catalog_blockers"][0]["blocker_code"]
        == "catalog_placeholder_pruned_no_article_record"
    )
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

    assert (
        main(
            [
                "--source-selection",
                str(selection),
                "--catalog",
                str(catalog),
                "--index",
                str(index),
                "--output",
                str(output),
            ]
        )
        == 0
    )

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

    assert (
        main(
            [
                "--source-selection",
                str(selection),
                "--catalog",
                str(catalog),
                "--index",
                str(index),
                "--output",
                str(tmp_path / "out.json"),
            ]
        )
        == 2
    )


def test_build_m031_rejects_duplicate_normalized_identities(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    payload["refs"].append(dict(payload["refs"][0], ref_id="duplicate"))
    _write_json(selection, payload)

    assert (
        main(
            [
                "--source-selection",
                str(selection),
                "--catalog",
                str(catalog),
                "--index",
                str(index),
                "--output",
                str(tmp_path / "out.json"),
            ]
        )
        == 2
    )


def test_build_m031_rejects_missing_index_rows_for_cataloged_refs(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    index_payload = json.loads(index.read_text(encoding="utf-8"))
    index_payload["articles"] = []
    _write_json(index, index_payload)

    assert (
        main(
            [
                "--source-selection",
                str(selection),
                "--catalog",
                str(catalog),
                "--index",
                str(index),
                "--output",
                str(tmp_path / "out.json"),
            ]
        )
        == 2
    )


def test_build_m031_rejects_path_traversal_in_index_article_path(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    index_payload = json.loads(index.read_text(encoding="utf-8"))
    index_payload["articles"][0]["article_path"] = "../escape/article.json"
    _write_json(index, index_payload)

    assert (
        main(
            [
                "--source-selection",
                str(selection),
                "--catalog",
                str(catalog),
                "--index",
                str(index),
                "--output",
                str(tmp_path / "out.json"),
            ]
        )
        == 2
    )


def test_build_m031_rejects_url_as_index_article_path(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    index_payload = json.loads(index.read_text(encoding="utf-8"))
    index_payload["articles"][0]["article_path"] = "https://example.test/article.json"
    _write_json(index, index_payload)

    assert (
        main(
            [
                "--source-selection",
                str(selection),
                "--catalog",
                str(catalog),
                "--index",
                str(index),
                "--output",
                str(tmp_path / "out.json"),
            ]
        )
        == 2
    )


def test_build_m031_rejects_true_unsafe_safety_flags(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    payload["refs"][0]["unsafe_claims"]["graph_ready_claimed"] = True
    _write_json(selection, payload)

    assert (
        main(
            [
                "--source-selection",
                str(selection),
                "--catalog",
                str(catalog),
                "--index",
                str(index),
                "--output",
                str(tmp_path / "out.json"),
            ]
        )
        == 2
    )


def test_build_m031_rejects_unsafe_source_variant_paths(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)
    article_path = (
        tmp_path
        / "article_catalog"
        / "article_catalog"
        / "arxiv"
        / "mixed-source"
        / "2605.29548"
        / "article.json"
    )
    article_payload = json.loads(article_path.read_text(encoding="utf-8"))
    article_payload["source_variants"][0]["path"] = "../escape.html"
    _write_json(article_path, article_payload)

    assert (
        main(
            [
                "--source-selection",
                str(selection),
                "--catalog",
                str(catalog),
                "--index",
                str(index),
                "--output",
                str(tmp_path / "out.json"),
            ]
        )
        == 2
    )


def test_build_m031_exposes_typed_blocker_diagnostics(tmp_path: Path) -> None:
    selection, catalog, index, _ = _fixture_tree(tmp_path)

    payload = build_selection(selection, catalog, index)

    blocker_diagnostics = [
        row for row in payload["diagnostics"] if row["identity"] == "arxiv:2605.26099"
    ]
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


def _replay_fixture_tree(tmp_path: Path) -> tuple[Path, Path, Path]:
    catalog_root = tmp_path / "article_catalog"
    article_path = (
        catalog_root / "article_catalog" / "arxiv" / "cs-cl" / "2507.19457" / "article.json"
    )
    source_dir = article_path.parent / "source"
    source_dir.mkdir(parents=True, exist_ok=True)
    (source_dir / "article.html").write_bytes(b"fixture article html bytes")
    (source_dir / "original.pdf").write_bytes(b"%fixture pdf bytes")
    (source_dir / "abs.html").write_bytes(b"fixture abs page bytes")
    _write_json(article_path, {"schema_version": "article.v00.01"})
    selection_path = tmp_path / "selection.json"
    _write_json(
        selection_path,
        {
            "schema_version": "m031-catalog-backed-replay-selection.v1",
            "selection_id": "m031-catalog-backed-replay-v1",
            "milestone_id": "M031-vwpd8e",
            "slice_id": "S02",
            "counts": {
                "requested_ref_count": 4,
                "catalog_backed_count": 3,
                "typed_catalog_blocker_count": 1,
                "silent_missing_count": 0,
                "source_variant_count": 6,
            },
            "requested_refs": [
                {
                    "ref_id": "m029-ref-001",
                    "identity": "arxiv:2507.19457",
                    "url": "https://arxiv.org/abs/2507.19457",
                },
                {
                    "ref_id": "m029-ref-002",
                    "identity": "stanford:cs224n:gradient-notes",
                    "url": "https://web.stanford.edu/class/cs224n/readings/gradient-notes.pdf",
                },
                {
                    "ref_id": "m029-ref-003",
                    "identity": "arxiv:2605.29548",
                    "url": "https://arxiv.org/abs/2605.29548",
                },
                {
                    "ref_id": "m029-ref-004",
                    "identity": "arxiv:2605.26099",
                    "url": "https://arxiv.org/abs/2605.26099",
                },
            ],
            "safety_flags": {
                "metadata_only_selection": True,
                "network_fetch_attempted": False,
                "source_acquisition_completed": False,
                "raw_article_text_embedded": False,
                "raw_article_html_embedded": False,
                "raw_pdf_bytes_embedded": False,
                "binary_payload_embedded": False,
                "base64_payload_embedded": False,
                "parser_ready_claimed": False,
                "chunk_ready_claimed": False,
                "kg_readiness_claimed": False,
                "graph_import_allowed": False,
                "production_ladybugdb_write_allowed": False,
                "trusted_kg_import_allowed": False,
                "production_import_attempted": False,
                "ladybugdb_written": False,
                "graph_write_attempted": False,
                "production_persistence_attempted": False,
            },
            "articles": [
                {
                    "identity": "arxiv:2507.19457",
                    "requested_ref_id": "m029-ref-001",
                    "requested_url": "https://arxiv.org/abs/2507.19457",
                    "article_ref": "arxiv/cs-cl/2507.19457",
                    "article_key": "2507.19457",
                    "article_path": "article_catalog/arxiv/cs-cl/2507.19457/article.json",
                    "source_variants": [
                        {
                            "variant_id": "html",
                            "source_role": "arxiv_html",
                            "local_path": "source/article.html",
                            "url": "https://arxiv.org/html/2507.19457v2",
                            "media_type": "text/html",
                            "is_metadata_only": False,
                            "requires_conversion": False,
                        },
                        {
                            "variant_id": "pdf",
                            "source_role": "arxiv_pdf",
                            "local_path": "source/original.pdf",
                            "url": "https://arxiv.org/pdf/2507.19457",
                            "media_type": "application/pdf",
                            "is_metadata_only": False,
                            "requires_conversion": True,
                        },
                        {
                            "variant_id": "abs",
                            "source_role": "arxiv_abs_page",
                            "local_path": "source/abs.html",
                            "url": "https://arxiv.org/abs/2507.19457",
                            "media_type": "text/html",
                            "is_metadata_only": True,
                            "requires_conversion": False,
                        },
                    ],
                },
                {
                    "identity": "stanford:cs224n:gradient-notes",
                    "requested_ref_id": "m029-ref-002",
                    "requested_url": "https://web.stanford.edu/class/cs224n/readings/gradient-notes.pdf",
                    "article_ref": "stanford/cs224n/gradient-notes",
                    "article_key": "gradient-notes",
                    "article_path": "article_catalog/stanford/cs224n/gradient-notes/article.json",
                    "source_variants": [
                        {
                            "variant_id": "external-pdf",
                            "source_role": "external_pdf",
                            "local_path": None,
                            "url": "https://web.stanford.edu/class/cs224n/readings/gradient-notes.pdf",
                            "media_type": "application/pdf",
                            "is_metadata_only": False,
                            "requires_conversion": True,
                        }
                    ],
                },
                {
                    "identity": "arxiv:2605.29548",
                    "requested_ref_id": "m029-ref-003",
                    "requested_url": "https://arxiv.org/abs/2605.29548",
                    "article_ref": "arxiv/mixed-source/2605.29548",
                    "article_key": "2605.29548",
                    "article_path": "article_catalog/arxiv/mixed-source/2605.29548/article.json",
                    "source_variants": [
                        {
                            "variant_id": "abs-metadata",
                            "source_role": "arxiv_abs_page",
                            "local_path": None,
                            "url": "https://arxiv.org/abs/2605.29548",
                            "media_type": "text/html",
                            "is_metadata_only": True,
                            "requires_conversion": False,
                        },
                        {
                            "variant_id": "pdf-metadata",
                            "source_role": "arxiv_pdf",
                            "local_path": None,
                            "url": "https://arxiv.org/pdf/2605.29548",
                            "media_type": "application/pdf",
                            "is_metadata_only": False,
                            "requires_conversion": True,
                        },
                    ],
                },
            ],
            "catalog_blockers": [
                {
                    "identity": "arxiv:2605.26099",
                    "requested_ref_id": "m029-ref-004",
                    "requested_url": "https://arxiv.org/abs/2605.26099",
                    "blocker_code": "catalog_placeholder_pruned_no_article_record",
                    "evidence": "index has no article.json-backed row",
                    "source_role": "arxiv_abs_url",
                }
            ],
        },
    )
    return selection_path, catalog_root, tmp_path / "source"


def test_replay_m031_real_acquisition_captures_local_artifacts_and_blocks_metadata_rows(
    tmp_path: Path,
) -> None:
    output_dir = tmp_path / "source"
    summary_path = tmp_path / "source-acquisition-summary.json"
    diagnostics_path = tmp_path / "source-acquisition-diagnostics.jsonl"
    report_path = tmp_path / "source-acquisition-report.md"

    result = subprocess.run(
        [
            sys.executable,
            str(REPLAY_SCRIPT),
            "--selection",
            "data/article_corpora/m031-catalog-backed-replay-v1/selection.json",
            "--catalog-root",
            "data/article_catalog",
            "--output-dir",
            str(output_dir),
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
    assert summary["counts"] == {"captured": 3, "blocked": 4, "failed": 0}
    assert summary["network_fetch_attempted_count"] == 0
    assert summary["graph_import_allowed"] is False
    assert summary["production_ladybugdb_write_allowed"] is False
    assert summary["ladybugdb_written"] is False
    captured = [row for row in summary["results"] if row["status"] == "captured"]
    assert {row["local_path"] for row in captured} == {
        "arxiv/cs-cl/2507.19457/source/article.html",
        "arxiv/cs-cl/2507.19457/source/original.pdf",
        "arxiv/cs-cl/2507.19457/source/abs.html",
    }
    for row in captured:
        artifact = output_dir / row["local_path"]
        assert artifact.exists()
        assert row["sha256"] == sha256_file(artifact)
        assert row["byte_size"] == artifact.stat().st_size
    blocked_codes = [
        row["diagnostic_code"] for row in summary["results"] if row["status"] == "blocked"
    ]
    assert blocked_codes.count("missing_local_source_path") == 3
    assert "catalog_placeholder_pruned_no_article_record" in blocked_codes
    serialized = (
        summary_path.read_text(encoding="utf-8")
        + diagnostics_path.read_text(encoding="utf-8")
        + report_path.read_text(encoding="utf-8")
    )
    assert "<html" not in serialized.lower()
    assert "</html" not in serialized.lower()
    assert "base64," not in serialized.lower()


def test_replay_m031_fixture_handles_null_external_pdf_and_typed_blocker(tmp_path: Path) -> None:
    selection, catalog_root, output_dir = _replay_fixture_tree(tmp_path)

    results = replay_selection(
        selection_path=selection, catalog_root=catalog_root, output_dir=output_dir
    )

    assert [row["status"] for row in results].count("captured") == 3
    assert [row["status"] for row in results].count("blocked") == 4
    external = next(row for row in results if row["identity"] == "stanford:cs224n:gradient-notes")
    assert external["source_role"] == "external_pdf"
    assert external["diagnostic_code"] == "missing_local_source_path"
    blocker = next(row for row in results if row["identity"] == "arxiv:2605.26099")
    assert blocker["diagnostic_code"] == "catalog_placeholder_pruned_no_article_record"
    assert blocker["article_ref"] is None


def test_replay_m031_blocks_unsafe_catalog_source_path(tmp_path: Path) -> None:
    selection, catalog_root, output_dir = _replay_fixture_tree(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    payload["articles"][0]["source_variants"][0]["local_path"] = "../escape.html"
    _write_json(selection, payload)

    results = replay_selection(
        selection_path=selection, catalog_root=catalog_root, output_dir=output_dir
    )

    unsafe = next(row for row in results if row["variant_id"] == "html")
    assert unsafe["status"] == "blocked"
    assert unsafe["diagnostic_code"] == "unsafe_catalog_source_path"
    assert not (output_dir / "arxiv" / "cs-cl" / "2507.19457" / "source" / "article.html").exists()


def test_replay_m031_blocks_absent_local_source_artifact(tmp_path: Path) -> None:
    selection, catalog_root, output_dir = _replay_fixture_tree(tmp_path)
    (
        catalog_root
        / "article_catalog"
        / "arxiv"
        / "cs-cl"
        / "2507.19457"
        / "source"
        / "article.html"
    ).unlink()

    results = replay_selection(
        selection_path=selection, catalog_root=catalog_root, output_dir=output_dir
    )

    missing = next(row for row in results if row["variant_id"] == "html")
    assert missing["status"] == "blocked"
    assert missing["diagnostic_code"] == "local_source_missing"
    assert missing["local_path"] == "arxiv/cs-cl/2507.19457/source/article.html"


def test_replay_m031_fails_empty_local_source_artifact(tmp_path: Path) -> None:
    selection, catalog_root, output_dir = _replay_fixture_tree(tmp_path)
    (
        catalog_root
        / "article_catalog"
        / "arxiv"
        / "cs-cl"
        / "2507.19457"
        / "source"
        / "article.html"
    ).write_bytes(b"")

    results = replay_selection(
        selection_path=selection, catalog_root=catalog_root, output_dir=output_dir
    )

    empty = next(row for row in results if row["variant_id"] == "html")
    assert empty["status"] == "failed"
    assert empty["diagnostic_code"] == "empty_local_source"


def test_replay_m031_cli_writes_redacted_summary_diagnostics_and_report(tmp_path: Path) -> None:
    selection, catalog_root, output_dir = _replay_fixture_tree(tmp_path)
    summary_path = tmp_path / "summary.json"
    diagnostics_path = tmp_path / "diagnostics.jsonl"
    report_path = tmp_path / "report.md"

    assert (
        replay_main(
            [
                "--selection",
                str(selection),
                "--catalog-root",
                str(catalog_root),
                "--output-dir",
                str(output_dir),
                "--write-summary",
                str(summary_path),
                "--write-diagnostics",
                str(diagnostics_path),
                "--write-report",
                str(report_path),
            ]
        )
        == 0
    )

    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["counts"] == {"captured": 3, "blocked": 4, "failed": 0}
    assert summary["fail_closed_safety_flags"]["graph_import_allowed"] is False
    assert "missing_local_source_path" in diagnostics_path.read_text(encoding="utf-8")
    report = report_path.read_text(encoding="utf-8")
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report
    serialized = (
        summary_path.read_text(encoding="utf-8")
        + diagnostics_path.read_text(encoding="utf-8")
        + report
    )
    assert "<html" not in serialized.lower()
    assert "</html" not in serialized.lower()
    assert "base64," not in serialized.lower()


def _fixture_acquisition_summary(tmp_path: Path) -> tuple[Path, Path, Path]:
    selection, catalog_root, output_dir = _replay_fixture_tree(tmp_path)
    results = replay_selection(
        selection_path=selection, catalog_root=catalog_root, output_dir=output_dir
    )
    summary_path = tmp_path / "source-acquisition-summary.json"
    _write_json(
        summary_path,
        {
            "schema_version": "m031-catalog-backed-acquisition.v1",
            "selection_id": "m031-catalog-backed-replay-v1",
            "results": results,
        },
    )
    return selection, summary_path, output_dir


def test_replay_m031_real_loader_evidence_only_loads_captured_artifacts(tmp_path: Path) -> None:
    summary_path = tmp_path / "loader-evidence-summary.json"
    diagnostics_path = tmp_path / "loader-evidence-diagnostics.jsonl"
    report_path = tmp_path / "loader-evidence-report.md"
    evidence_dir = tmp_path / "loader-evidence"

    result = subprocess.run(
        [
            sys.executable,
            str(LOADER_SCRIPT),
            "--selection",
            "data/article_corpora/m031-catalog-backed-replay-v1/selection.json",
            "--acquisition-summary",
            "data/article_corpora/m031-catalog-backed-replay-v1/source-acquisition-summary.json",
            "--source-dir",
            "data/article_corpora/m031-catalog-backed-replay-v1/source",
            "--output-dir",
            str(evidence_dir),
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
    assert summary["counts"] == {
        "loader_attempted": 3,
        "loaded": 2,
        "loaded_metadata_only": 1,
        "failed": 0,
        "loader_blocked": 4,
    }
    assert summary["network_fetch_attempted_count"] == 0
    assert summary["graph_import_allowed"] is False
    assert summary["production_ladybugdb_write_allowed"] is False
    assert summary["ladybugdb_written"] is False
    attempted = [row for row in summary["results"] if row["loader_attempted"]]
    assert {row["local_path"] for row in attempted} == {
        "arxiv/cs-cl/2507.19457/source/article.html",
        "arxiv/cs-cl/2507.19457/source/original.pdf",
        "arxiv/cs-cl/2507.19457/source/abs.html",
    }
    pdf_row = next(row for row in attempted if row["source_role"] == "arxiv_pdf")
    assert pdf_row["status"] == "loaded_metadata_only"
    assert pdf_row["text_present"] is False
    assert pdf_row["parser_name"] == "pdf_metadata_probe"
    html_rows = [row for row in attempted if row["source_role"] in {"arxiv_html", "arxiv_abs_page"}]
    assert all(row["status"] == "loaded" and row["text_present"] is True for row in html_rows)
    blockers = [row for row in summary["results"] if row["status"] == "blocked"]
    assert len(blockers) == 4
    assert all(row["loader_attempted"] is False for row in blockers)
    event_path = evidence_dir / "arxiv" / "cs-cl" / "2507.19457" / "events.jsonl"
    assert event_path.exists()
    events = [json.loads(line) for line in event_path.read_text(encoding="utf-8").splitlines()]
    assert (
        len(
            [
                event
                for event in events
                if event["event"] in {"source.load_completed", "source.load_failed"}
            ]
        )
        == 3
    )
    assert all(not Path(event["source_path"]).is_absolute() for event in events)
    assert {event["source_path"] for event in events} == {
        "arxiv/cs-cl/2507.19457/source/article.html",
        "arxiv/cs-cl/2507.19457/source/original.pdf",
        "arxiv/cs-cl/2507.19457/source/abs.html",
    }
    serialized = (
        summary_path.read_text(encoding="utf-8")
        + diagnostics_path.read_text(encoding="utf-8")
        + report_path.read_text(encoding="utf-8")
        + event_path.read_text(encoding="utf-8")
    )
    assert "GEPA: Reflective Prompt" not in serialized
    assert "base64," not in serialized.lower()
    assert '"text":' not in serialized
    assert '"raw_binary":' not in serialized
    report = report_path.read_text(encoding="utf-8")
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report


def test_replay_m031_fixture_loader_blocks_non_captured_rows_and_redacts_text(
    tmp_path: Path,
) -> None:
    selection, acquisition_summary, source_dir = _fixture_acquisition_summary(tmp_path)

    rows = replay_loader_evidence(
        selection_path=selection,
        acquisition_summary_path=acquisition_summary,
        source_dir=source_dir,
        output_dir=tmp_path / "loader-evidence",
    )

    assert [row["loader_attempted"] for row in rows].count(True) == 3
    assert [row["status"] for row in rows].count("blocked") == 4
    assert all(
        row["loader_attempted"] is False for row in rows if row["acquisition_status"] != "captured"
    )
    assert "fixture article html bytes" not in json.dumps(rows)
    assert all("text" not in row for row in rows)
    pdf_row = next(
        row for row in rows if row["source_role"] == "arxiv_pdf" and row["loader_attempted"]
    )
    assert pdf_row["status"] == "loaded_metadata_only"
    assert pdf_row["text_present"] is False


def test_replay_m031_loader_blocks_missing_captured_file(tmp_path: Path) -> None:
    selection, acquisition_summary, source_dir = _fixture_acquisition_summary(tmp_path)
    (source_dir / "arxiv" / "cs-cl" / "2507.19457" / "source" / "article.html").unlink()

    rows = replay_loader_evidence(
        selection_path=selection,
        acquisition_summary_path=acquisition_summary,
        source_dir=source_dir,
        output_dir=tmp_path / "loader-evidence",
    )

    missing = next(row for row in rows if row["source_role"] == "arxiv_html")
    assert missing["status"] == "blocked"
    assert missing["diagnostic_code"] == "captured_source_missing"
    assert missing["loader_attempted"] is False


def test_replay_m031_loader_blocks_hash_mismatch_before_loading(tmp_path: Path) -> None:
    selection, acquisition_summary, source_dir = _fixture_acquisition_summary(tmp_path)
    (source_dir / "arxiv" / "cs-cl" / "2507.19457" / "source" / "article.html").write_bytes(
        b"changed bytes"
    )

    rows = replay_loader_evidence(
        selection_path=selection,
        acquisition_summary_path=acquisition_summary,
        source_dir=source_dir,
        output_dir=tmp_path / "loader-evidence",
    )

    mismatch = next(row for row in rows if row["source_role"] == "arxiv_html")
    assert mismatch["status"] == "blocked"
    assert mismatch["diagnostic_code"] == "captured_hash_mismatch"
    assert mismatch["loader_attempted"] is False


def test_replay_m031_loader_confines_unsafe_event_article_ref(tmp_path: Path) -> None:
    selection, acquisition_summary, source_dir = _fixture_acquisition_summary(tmp_path)
    payload = json.loads(acquisition_summary.read_text(encoding="utf-8"))
    payload["results"][0]["article_ref"] = "../escape"
    _write_json(acquisition_summary, payload)

    rows = replay_loader_evidence(
        selection_path=selection,
        acquisition_summary_path=acquisition_summary,
        source_dir=source_dir,
        output_dir=tmp_path / "loader-evidence",
    )

    unsafe = next(row for row in rows if row["source_role"] == "arxiv_html")
    assert unsafe["loader_attempted"] is True
    assert unsafe["event_path"].startswith("unsafe/")
    assert ".." not in unsafe["event_path"]
    assert (tmp_path / "loader-evidence" / unsafe["event_path"]).exists()


def test_replay_m031_loader_rejects_malformed_acquisition_results(tmp_path: Path) -> None:
    selection, acquisition_summary, source_dir = _fixture_acquisition_summary(tmp_path)
    payload = json.loads(acquisition_summary.read_text(encoding="utf-8"))
    payload["results"] = {"not": "a-list"}
    _write_json(acquisition_summary, payload)

    with pytest.raises(LoaderEvidenceError, match="acquisition summary results must be a list"):
        replay_loader_evidence(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            source_dir=source_dir,
            output_dir=tmp_path / "loader-evidence",
        )


def test_replay_m031_loader_rejects_selection_acquisition_mismatch(tmp_path: Path) -> None:
    selection, acquisition_summary, source_dir = _fixture_acquisition_summary(tmp_path)
    payload = json.loads(acquisition_summary.read_text(encoding="utf-8"))
    payload["selection_id"] = "different-selection"
    _write_json(acquisition_summary, payload)

    with pytest.raises(LoaderEvidenceError, match="selection_id mismatch"):
        replay_loader_evidence(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            source_dir=source_dir,
            output_dir=tmp_path / "loader-evidence",
        )


def test_replay_m031_loader_rejects_true_fail_closed_safety_flags(tmp_path: Path) -> None:
    selection, acquisition_summary, source_dir = _fixture_acquisition_summary(tmp_path)
    rows = replay_loader_evidence(
        selection_path=selection,
        acquisition_summary_path=acquisition_summary,
        source_dir=source_dir,
        output_dir=tmp_path / "loader-evidence",
    )
    summary = build_loader_summary(
        rows,
        selection_path=selection,
        acquisition_summary_path=acquisition_summary,
        source_dir=source_dir,
        output_dir=tmp_path / "loader-evidence",
        duration_ms=0,
    )
    summary["fail_closed_safety_flags"]["graph_import_allowed"] = True

    with pytest.raises(LoaderEvidenceError, match="unexpected safety flag"):
        assert_fail_closed_flags(summary)


def test_replay_m031_loader_cli_writes_summary_diagnostics_report(tmp_path: Path) -> None:
    selection, acquisition_summary, source_dir = _fixture_acquisition_summary(tmp_path)
    summary_path = tmp_path / "summary.json"
    diagnostics_path = tmp_path / "diagnostics.jsonl"
    report_path = tmp_path / "report.md"

    assert (
        loader_main(
            [
                "--selection",
                str(selection),
                "--acquisition-summary",
                str(acquisition_summary),
                "--source-dir",
                str(source_dir),
                "--output-dir",
                str(tmp_path / "loader-evidence"),
                "--write-summary",
                str(summary_path),
                "--write-diagnostics",
                str(diagnostics_path),
                "--write-report",
                str(report_path),
            ]
        )
        == 0
    )

    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["counts"]["loader_attempted"] == 3
    assert summary["counts"]["loader_blocked"] == 4
    assert "missing_local_source_path" in diagnostics_path.read_text(encoding="utf-8")
    assert "## Failure Modes" in report_path.read_text(encoding="utf-8")


def _fixture_closeout_bundle(tmp_path: Path) -> tuple[Path, Path, Path, Path, Path]:
    selection, catalog_root, source_dir = _replay_fixture_tree(tmp_path)
    acquisition_rows = replay_selection(
        selection_path=selection, catalog_root=catalog_root, output_dir=source_dir
    )
    acquisition_summary = build_acquisition_summary(
        acquisition_rows,
        selection_path=selection,
        catalog_root=catalog_root,
        output_dir=source_dir,
        duration_ms=0,
    )
    acquisition_summary_path = tmp_path / "source-acquisition-summary.json"
    _write_json(acquisition_summary_path, acquisition_summary)
    loader_dir = tmp_path / "loader-evidence"
    loader_rows = replay_loader_evidence(
        selection_path=selection,
        acquisition_summary_path=acquisition_summary_path,
        source_dir=source_dir,
        output_dir=loader_dir,
    )
    loader_summary = build_loader_summary(
        loader_rows,
        selection_path=selection,
        acquisition_summary_path=acquisition_summary_path,
        source_dir=source_dir,
        output_dir=loader_dir,
        duration_ms=0,
    )
    loader_summary_path = tmp_path / "loader-evidence-summary.json"
    _write_json(loader_summary_path, loader_summary)
    return selection, acquisition_summary_path, loader_summary_path, source_dir, loader_dir


def test_verify_m031_closeout_cli_writes_passed_summary_diagnostics_and_report(
    tmp_path: Path,
) -> None:
    summary_path = tmp_path / "replay-closeout-summary.json"
    diagnostics_path = tmp_path / "replay-closeout-diagnostics.jsonl"
    report_path = tmp_path / "replay-closeout-report.md"

    result = subprocess.run(
        [
            sys.executable,
            str(CLOSEOUT_SCRIPT),
            "--selection",
            "data/article_corpora/m031-catalog-backed-replay-v1/selection.json",
            "--acquisition-summary",
            "data/article_corpora/m031-catalog-backed-replay-v1/source-acquisition-summary.json",
            "--loader-summary",
            "data/article_corpora/m031-catalog-backed-replay-v1/loader-evidence-summary.json",
            "--source-dir",
            "data/article_corpora/m031-catalog-backed-replay-v1/source",
            "--loader-dir",
            "data/article_corpora/m031-catalog-backed-replay-v1/loader-evidence",
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
    assert summary["status"] == "passed"
    assert summary["requested_ref_count"] == 4
    assert summary["catalog_backed_count"] == 3
    assert summary["typed_catalog_blocker_count"] == 1
    assert summary["counts"]["captured_acquisition_rows"] == 3
    assert summary["counts"]["loader_attempted"] == 3
    assert summary["counts"]["loader_blocked"] == 4
    assert "closeout_contract_passed" in diagnostics_path.read_text(encoding="utf-8")
    report = report_path.read_text(encoding="utf-8")
    assert "Parser readiness, conversion readiness, chunk readiness" in report
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report
    serialized = (
        summary_path.read_text(encoding="utf-8")
        + diagnostics_path.read_text(encoding="utf-8")
        + report
    )
    assert "<html" not in serialized.lower()
    assert "base64," not in serialized.lower()
    assert "GEPA: Reflective Prompt" not in serialized


def test_verify_m031_closeout_rejects_omitted_blocked_row(tmp_path: Path) -> None:
    selection, acquisition_summary, loader_summary, source_dir, loader_dir = (
        _fixture_closeout_bundle(tmp_path)
    )
    payload = json.loads(loader_summary.read_text(encoding="utf-8"))
    payload["results"] = [
        row for row in payload["results"] if row["identity"] != "arxiv:2605.26099"
    ]
    payload["counts"] = {
        "loader_attempted": 3,
        "loaded": 2,
        "loaded_metadata_only": 1,
        "failed": 0,
        "loader_blocked": 3,
    }
    _write_json(loader_summary, payload)

    with pytest.raises(CloseoutError, match="expected 7 terminal rows"):
        verify_contract(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            loader_summary_path=loader_summary,
            source_dir=source_dir,
            loader_dir=loader_dir,
        )


def test_verify_m031_closeout_rejects_selected_variant_without_acquisition_state(
    tmp_path: Path,
) -> None:
    selection, acquisition_summary, loader_summary, source_dir, loader_dir = (
        _fixture_closeout_bundle(tmp_path)
    )
    payload = json.loads(acquisition_summary.read_text(encoding="utf-8"))
    payload["results"] = [row for row in payload["results"] if row["variant_id"] != "abs"]
    payload["counts"] = {"captured": 2, "blocked": 4, "failed": 0}
    _write_json(acquisition_summary, payload)

    with pytest.raises(CloseoutError, match="expected 7 terminal rows"):
        verify_contract(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            loader_summary_path=loader_summary,
            source_dir=source_dir,
            loader_dir=loader_dir,
        )


def test_verify_m031_closeout_rejects_loader_blocker_missing(tmp_path: Path) -> None:
    selection, acquisition_summary, loader_summary, source_dir, loader_dir = (
        _fixture_closeout_bundle(tmp_path)
    )
    payload = json.loads(loader_summary.read_text(encoding="utf-8"))
    blocker = next(
        row for row in payload["results"] if row["identity"] == "stanford:cs224n:gradient-notes"
    )
    blocker["status"] = "loaded"
    blocker["loader_attempted"] = False
    blocker["counts"] = payload.get("counts")
    payload["counts"] = {
        "loader_attempted": 3,
        "loaded": 3,
        "loaded_metadata_only": 1,
        "failed": 0,
        "loader_blocked": 3,
    }
    _write_json(loader_summary, payload)

    with pytest.raises(CloseoutError, match="non-captured acquisition row is not a loader blocker"):
        verify_contract(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            loader_summary_path=loader_summary,
            source_dir=source_dir,
            loader_dir=loader_dir,
        )


def test_verify_m031_closeout_rejects_loader_acquisition_mismatch(tmp_path: Path) -> None:
    selection, acquisition_summary, loader_summary, source_dir, loader_dir = (
        _fixture_closeout_bundle(tmp_path)
    )
    payload = json.loads(loader_summary.read_text(encoding="utf-8"))
    attempted = next(row for row in payload["results"] if row["variant_id"] == "html")
    attempted["loader_attempted"] = False
    payload["counts"]["loader_attempted"] = 2
    _write_json(loader_summary, payload)

    with pytest.raises(
        CloseoutError, match="loader attempts do not match captured acquisition rows"
    ):
        verify_contract(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            loader_summary_path=loader_summary,
            source_dir=source_dir,
            loader_dir=loader_dir,
        )


def test_verify_m031_closeout_rejects_loader_event_text_leakage(tmp_path: Path) -> None:
    selection, acquisition_summary, loader_summary, source_dir, loader_dir = (
        _fixture_closeout_bundle(tmp_path)
    )
    event_path = loader_dir / "arxiv" / "cs-cl" / "2507.19457" / "events.jsonl"
    event_path.write_text(
        '{"event":"source.load_completed","text":"fixture article html bytes"}\n', encoding="utf-8"
    )

    with pytest.raises(CloseoutError, match="metadata artifact is not redacted"):
        verify_contract(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            loader_summary_path=loader_summary,
            source_dir=source_dir,
            loader_dir=loader_dir,
        )


@pytest.mark.parametrize(
    ("flag", "value"),
    [
        ("graph_import_allowed", True),
        ("production_import_attempted", True),
        ("ladybugdb_written", True),
    ],
)
def test_verify_m031_closeout_rejects_unsafe_true_flags(
    tmp_path: Path, flag: str, value: bool
) -> None:
    selection, acquisition_summary, loader_summary, source_dir, loader_dir = (
        _fixture_closeout_bundle(tmp_path)
    )
    payload = json.loads(loader_summary.read_text(encoding="utf-8"))
    payload[flag] = value
    _write_json(loader_summary, payload)

    with pytest.raises(CloseoutError, match="unsafe safety flag"):
        verify_contract(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            loader_summary_path=loader_summary,
            source_dir=source_dir,
            loader_dir=loader_dir,
        )


def test_verify_m031_closeout_rejects_stale_hash(tmp_path: Path) -> None:
    selection, acquisition_summary, loader_summary, source_dir, loader_dir = (
        _fixture_closeout_bundle(tmp_path)
    )
    (source_dir / "arxiv" / "cs-cl" / "2507.19457" / "source" / "article.html").write_bytes(
        b"changed bytes"
    )

    with pytest.raises(CloseoutError, match="captured hash mismatch"):
        verify_contract(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            loader_summary_path=loader_summary,
            source_dir=source_dir,
            loader_dir=loader_dir,
        )


def test_verify_m031_closeout_rejects_path_escape(tmp_path: Path) -> None:
    selection, acquisition_summary, loader_summary, source_dir, loader_dir = (
        _fixture_closeout_bundle(tmp_path)
    )
    payload = json.loads(loader_summary.read_text(encoding="utf-8"))
    payload["results"][0]["event_path"] = "../escape/events.jsonl"
    _write_json(loader_summary, payload)

    with pytest.raises(CloseoutError, match="unsafe relative path"):
        verify_contract(
            selection_path=selection,
            acquisition_summary_path=acquisition_summary,
            loader_summary_path=loader_summary,
            source_dir=source_dir,
            loader_dir=loader_dir,
        )
