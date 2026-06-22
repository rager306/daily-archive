from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

# pyrefly: ignore [missing-import]
from capture_m029_unified_sources import (  # noqa: E402  # ty:ignore[unresolved-import]
    capture_selection,
    sha256_file,
)

# pyrefly: ignore [missing-import]
from verify_m029_unified_source_acquisition import (  # ty: ignore[unresolved-import]
    main as verify_main,  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
)

CAPTURE_SCRIPT = Path(__file__).parents[1] / "scripts" / "capture_m029_unified_sources.py"


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _fixture_tree(tmp_path: Path) -> tuple[Path, Path, Path, Path]:
    catalog_root = tmp_path / "article_catalog"
    article_path = (
        catalog_root / "article_catalog" / "arxiv" / "mixed-source" / "2605.20897" / "article.json"
    )
    source_path = article_path.parent / "source" / "abs.html"
    source_path.parent.mkdir(parents=True, exist_ok=True)
    source_path.write_bytes(b"fixture arxiv abstract bytes")
    _write_json(
        article_path,
        {
            "schema_version": "article.v00.01",
            "article_key": "2605.20897",
            "catalog_path": "arxiv/mixed-source/2605.20897",
            "source_variants": [
                {
                    "variant_id": "2605.20897:source:arxiv-abs",
                    "source_role": "arxiv_abs_page",
                    "path": "source/abs.html",
                    "local_path": "source/abs.html",
                    "url": "https://arxiv.org/abs/2605.20897",
                    "media_type": "text/html",
                    "raw_text_embedded": False,
                    "raw_binary_embedded": False,
                }
            ],
        },
    )
    catalog_path = catalog_root / "catalog.json"
    index_path = catalog_root / "index.json"
    selection_path = tmp_path / "corpus" / "selection.json"
    _write_json(catalog_path, {"schema_version": "article-catalog.v00.01"})
    _write_json(
        index_path,
        {
            "schema_version": "article-catalog-index.v00.01",
            "articles": [
                {
                    "article_ref": "arxiv/mixed-source/2605.20897",
                    "article_key": "2605.20897",
                    "article_path": "article_catalog/arxiv/mixed-source/2605.20897/article.json",
                    "primary_source_role": "arxiv_abs_page",
                }
            ],
        },
    )
    _write_json(
        selection_path,
        {
            "schema_version": "article-corpus-selection.v00.01",
            "selection_id": "m029-unified-corpus-v1",
            "network_policy": {"capture_phase_may_fetch": False},
            "articles": [
                {
                    "article_ref": "arxiv/mixed-source/2605.20897",
                    "article_key": "2605.20897",
                    "identity_key": "arxiv:2605.20897",
                    "source_code": "arxiv",
                    "source_strategy": "arxiv_abs_page",
                    "catalog_resolution": "resolved",
                    "seed_url": "https://arxiv.org/abs/2605.20897",
                    "canonical_url": "https://arxiv.org/abs/2605.20897",
                },
                {
                    "article_key": "2605.23904",
                    "identity_key": "arxiv:2605.23904",
                    "source_code": "arxiv",
                    "source_strategy": "arxiv_abs_page",
                    "catalog_resolution": "unresolved",
                    "seed_url": "https://arxiv.org/abs/2605.23904",
                    "canonical_url": "https://arxiv.org/abs/2605.23904",
                },
            ],
            "safety_flags": {
                "graph_import_allowed": False,
                "production_ladybugdb_write_allowed": False,
                "trusted_kg_import_allowed": False,
                "production_import_attempted": False,
                "ladybugdb_written": False,
            },
        },
    )
    return catalog_path, index_path, selection_path, tmp_path / "corpus" / "source"


def test_capture_m029_copies_local_sources_and_blocks_unresolved_placeholders(
    tmp_path: Path,
) -> None:
    catalog_path, index_path, selection_path, output_dir = _fixture_tree(tmp_path)

    results = capture_selection(
        catalog_root=catalog_path.parent,
        index_path=index_path,
        selection_path=selection_path,
        output_dir=output_dir,
    )

    captured = [row for row in results if row["status"] == "captured"]
    blocked = [row for row in results if row["status"] == "blocked"]
    assert len(captured) == 1
    assert len(blocked) == 1
    assert captured[0]["network_fetch_attempted"] is False
    artifact = output_dir / captured[0]["local_path"]
    assert artifact.read_bytes() == b"fixture arxiv abstract bytes"
    assert captured[0]["sha256"] == sha256_file(artifact)
    assert blocked[0]["diagnostic_code"] == "catalog_unresolved"
    assert blocked[0]["url"] == "https://arxiv.org/abs/2605.23904"


def test_capture_cli_and_verify_write_metadata_only_artifacts(tmp_path: Path) -> None:
    catalog_path, index_path, selection_path, output_dir = _fixture_tree(tmp_path)

    capture = subprocess.run(
        [
            sys.executable,
            str(CAPTURE_SCRIPT),
            "--selection",
            str(selection_path),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--output-dir",
            str(output_dir),
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert capture.returncode == 0, capture.stderr

    summary_path = output_dir.parent / "source-acquisition-summary.json"
    diagnostics_path = output_dir.parent / "source-acquisition-diagnostics.jsonl"
    report_path = output_dir.parent / "source-acquisition-report.md"
    result = verify_main(
        [
            "verify_m029_unified_source_acquisition.py",
            "--selection",
            str(selection_path),
            "--source-dir",
            str(output_dir),
            "--write-summary",
            str(summary_path),
            "--write-diagnostics",
            str(diagnostics_path),
            "--write-report",
            str(report_path),
            "--require-no-network",
            "--require-no-import-flags",
        ]
    )

    assert result == 0
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["status"] == "passed"
    assert summary["counts"] == {"captured": 1, "blocked": 1, "failed": 0}
    assert summary["network_fetch_attempted_count"] == 0
    diagnostics = diagnostics_path.read_text(encoding="utf-8")
    assert "catalog_unresolved" in diagnostics
    assert "https://arxiv.org/abs/2605.23904" in diagnostics
    assert "<html" not in report_path.read_text(encoding="utf-8")


def test_verify_check_strategies_writes_primary_fallback_policy_summary(tmp_path: Path) -> None:
    catalog_path, index_path, selection_path, output_dir = _fixture_tree(tmp_path)
    capture = subprocess.run(
        [
            sys.executable,
            str(CAPTURE_SCRIPT),
            "--selection",
            str(selection_path),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--output-dir",
            str(output_dir),
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert capture.returncode == 0, capture.stderr

    summary_path = output_dir.parent / "source-strategy-summary.json"
    diagnostics_path = output_dir.parent / "source-strategy-diagnostics.jsonl"
    result = verify_main(
        [
            "verify_m029_unified_source_acquisition.py",
            "--selection",
            str(selection_path),
            "--source-dir",
            str(output_dir),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--check-strategies",
            "--check-fail-closed",
            "--write-summary",
            str(summary_path),
            "--write-diagnostics",
            str(diagnostics_path),
        ]
    )

    assert result == 0
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["schema_version"] == "m029-source-strategy-normalization.v1"
    assert summary["status"] == "passed"
    assert summary["counts"]["articles"] == 2
    assert summary["counts"]["primary_captured"] == 1
    assert summary["counts"]["primary_blocked"] == 1
    assert summary["counts"]["fallback_needed"] == 0
    resolved = next(row for row in summary["articles"] if row["article_key"] == "2605.20897")
    assert resolved["intended_primary_source_role"] == "arxiv_abs_page"
    assert resolved["content_fallback_roles"] == []
    assert resolved["capture_policy"] == "local_only_no_network"
    assert resolved["primary_terminal_state"] == "captured"
    unresolved = next(row for row in summary["articles"] if row["article_key"] == "2605.23904")
    assert unresolved["primary_terminal_state"] == "blocked"
    assert diagnostics_path.read_text(encoding="utf-8") == ""


def test_verify_check_strategies_rejects_selection_catalog_primary_mismatch(tmp_path: Path) -> None:
    catalog_path, index_path, selection_path, output_dir = _fixture_tree(tmp_path)
    capture = subprocess.run(
        [
            sys.executable,
            str(CAPTURE_SCRIPT),
            "--selection",
            str(selection_path),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--output-dir",
            str(output_dir),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    assert capture.returncode == 0
    index = json.loads(index_path.read_text(encoding="utf-8"))
    index["articles"][0]["primary_source_role"] = "arxiv_pdf"
    _write_json(index_path, index)

    result = verify_main(
        [
            "verify_m029_unified_source_acquisition.py",
            "--selection",
            str(selection_path),
            "--source-dir",
            str(output_dir),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--check-strategies",
            "--write-summary",
            str(output_dir.parent / "source-strategy-summary.json"),
            "--write-diagnostics",
            str(output_dir.parent / "source-strategy-diagnostics.jsonl"),
        ]
    )

    assert result == 1
    diagnostics = (output_dir.parent / "source-strategy-diagnostics.jsonl").read_text(
        encoding="utf-8"
    )
    assert "strategy_primary_mismatch" in diagnostics


def test_capture_blocks_unsafe_catalog_source_path(tmp_path: Path) -> None:
    catalog_path, index_path, selection_path, output_dir = _fixture_tree(tmp_path)
    article_path = (
        catalog_path.parent
        / "article_catalog"
        / "arxiv"
        / "mixed-source"
        / "2605.20897"
        / "article.json"
    )
    article = json.loads(article_path.read_text(encoding="utf-8"))
    article["source_variants"][0]["local_path"] = "../outside.html"
    article["source_variants"][0]["path"] = "../outside.html"
    _write_json(article_path, article)

    results = capture_selection(
        catalog_root=catalog_path.parent,
        index_path=index_path,
        selection_path=selection_path,
        output_dir=output_dir,
    )

    first = results[0]
    assert first["status"] == "blocked"
    assert first["diagnostic_code"] == "unsafe_catalog_source_path"
    assert not (
        output_dir / "arxiv" / "mixed-source" / "2605.20897" / "source" / "abs.html"
    ).exists()


def test_verifier_detects_hash_mismatch(tmp_path: Path) -> None:
    catalog_path, index_path, selection_path, output_dir = _fixture_tree(tmp_path)
    capture = subprocess.run(
        [
            sys.executable,
            str(CAPTURE_SCRIPT),
            "--selection",
            str(selection_path),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--output-dir",
            str(output_dir),
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert capture.returncode == 0, capture.stderr
    artifact = output_dir / "arxiv" / "mixed-source" / "2605.20897" / "source" / "abs.html"
    artifact.write_bytes(b"tampered")

    result = verify_main(
        [
            "verify_m029_unified_source_acquisition.py",
            "--selection",
            str(selection_path),
            "--source-dir",
            str(output_dir),
            "--write-summary",
            str(output_dir.parent / "source-acquisition-summary.json"),
            "--write-diagnostics",
            str(output_dir.parent / "source-acquisition-diagnostics.jsonl"),
            "--write-report",
            str(output_dir.parent / "source-acquisition-report.md"),
            "--require-no-network",
            "--require-no-import-flags",
        ]
    )

    assert result == 1
    diagnostics = (output_dir.parent / "source-acquisition-diagnostics.jsonl").read_text(
        encoding="utf-8"
    )
    assert "sha256_mismatch" in diagnostics


@pytest.mark.parametrize("bad_url", [None, ""])
def test_selection_url_must_be_present(tmp_path: Path, bad_url: str | None) -> None:
    catalog_path, index_path, selection_path, output_dir = _fixture_tree(tmp_path)
    selection = json.loads(selection_path.read_text(encoding="utf-8"))
    selection["articles"][1]["seed_url"] = bad_url
    selection["articles"][1].pop("canonical_url", None)
    _write_json(selection_path, selection)

    subprocess.run(
        [
            sys.executable,
            str(CAPTURE_SCRIPT),
            "--selection",
            str(selection_path),
            "--catalog",
            str(catalog_path),
            "--index",
            str(index_path),
            "--output-dir",
            str(output_dir),
        ],
        check=True,
        text=True,
        capture_output=True,
    )

    with pytest.raises(ValueError, match="selection row missing seed/canonical URL"):
        verify_main(
            [
                "verify_m029_unified_source_acquisition.py",
                "--selection",
                str(selection_path),
                "--source-dir",
                str(output_dir),
                "--write-summary",
                str(output_dir.parent / "source-acquisition-summary.json"),
                "--write-diagnostics",
                str(output_dir.parent / "source-acquisition-diagnostics.jsonl"),
                "--write-report",
                str(output_dir.parent / "source-acquisition-report.md"),
            ]
        )
