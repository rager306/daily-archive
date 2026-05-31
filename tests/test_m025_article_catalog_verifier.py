from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

FIXTURE_ROOT = Path(__file__).parent / "fixtures" / "article_catalog_v00_01"
SCRIPT = Path(__file__).parents[1] / "scripts" / "verify_m025_article_catalog.py"


def _copy_scaffold(tmp_path: Path) -> tuple[Path, Path, Path]:
    catalog_dir = tmp_path / "article_catalog"
    corpus_dir = tmp_path / "article_corpora" / "m025-rlm-dspy-pageindex-smoke-v1"
    shutil.copytree(FIXTURE_ROOT / "article_catalog", catalog_dir / "article_catalog")
    shutil.copy2(FIXTURE_ROOT / "catalog.json", catalog_dir / "catalog.json")
    shutil.copy2(FIXTURE_ROOT / "article_catalog" / "index.json", catalog_dir / "index.json")
    shutil.copytree(
        FIXTURE_ROOT / "corpora" / "m025-rlm-dspy-pageindex-smoke-v1",
        corpus_dir,
    )
    schemas_dir = catalog_dir / "schemas"
    schemas_dir.mkdir()
    (schemas_dir / "article-catalog-schema.v00.01.json").write_text(
        json.dumps({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object"}),
        encoding="utf-8",
    )
    (schemas_dir / "article-schema.v00.01.json").write_text(
        json.dumps({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object"}),
        encoding="utf-8",
    )
    return catalog_dir / "catalog.json", catalog_dir / "index.json", corpus_dir / "selection.json"


def _run_verifier(catalog: Path, index: Path, selection: Path) -> subprocess.CompletedProcess[str]:
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
        ],
        capture_output=True,
        check=False,
        text=True,
    )


def _run_rebuild(
    catalog: Path,
    index: Path,
    selection: Path,
    report: Path,
    diagnostics: Path,
) -> subprocess.CompletedProcess[str]:
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
            "--rebuild-index",
            "--write-index",
            str(index),
            "--write-index-report",
            str(report),
            "--write-diagnostics",
            str(diagnostics),
            "--check-index-idempotent",
            "--check-index-titles",
            "--check-safe-traversal",
            "--check-duplicate-lookups",
            "--check-index-lookup-only",
        ],
        capture_output=True,
        check=False,
        text=True,
    )


def test_m025_article_catalog_verifier_accepts_fixture_scaffold(tmp_path: Path) -> None:
    catalog, index, selection = _copy_scaffold(tmp_path)

    result = _run_verifier(catalog, index, selection)

    assert result.returncode == 0, result.stderr
    assert "validation passed" in result.stdout


def test_m025_article_catalog_verifier_rejects_index_title_drift(tmp_path: Path) -> None:
    catalog, index, selection = _copy_scaffold(tmp_path)
    payload = json.loads(index.read_text(encoding="utf-8"))
    payload["articles"][0]["title"] = "Drifted title"
    index.write_text(json.dumps(payload), encoding="utf-8")

    result = _run_verifier(catalog, index, selection)

    assert result.returncode == 1
    assert "index_title" in result.stderr


def test_m025_article_catalog_verifier_rejects_selection_not_in_index(tmp_path: Path) -> None:
    catalog, index, selection = _copy_scaffold(tmp_path)
    payload = json.loads(selection.read_text(encoding="utf-8"))
    payload["articles"][0]["article_ref"] = "arxiv/cs-ai/missing"
    selection.write_text(json.dumps(payload), encoding="utf-8")

    result = _run_verifier(catalog, index, selection)

    assert result.returncode == 1
    assert "selection article_ref not present in index" in result.stderr


def test_m025_article_catalog_rebuild_writes_idempotent_report_and_diagnostics(tmp_path: Path) -> None:
    catalog, index, selection = _copy_scaffold(tmp_path)
    report = index.parent / "index-rebuild-report.json"
    diagnostics = index.parent / "index-rebuild-diagnostics.jsonl"

    result = _run_rebuild(catalog, index, selection, report, diagnostics)

    assert result.returncode == 0, result.stderr
    assert "index rebuild passed" in result.stdout
    rebuilt_report = json.loads(report.read_text(encoding="utf-8"))
    assert rebuilt_report["entries_emitted"] == 5
    assert rebuilt_report["idempotent"] is True
    assert rebuilt_report["network_fetch_attempted"] is False
    assert diagnostics.read_text(encoding="utf-8") == ""


def test_m025_article_catalog_rebuild_rejects_duplicate_lookup_key(tmp_path: Path) -> None:
    catalog, index, selection = _copy_scaffold(tmp_path)
    article_path = index.parent / "article_catalog" / "arxiv" / "cs-ai" / "2605.28617v1" / "article.json"
    article = json.loads(article_path.read_text(encoding="utf-8"))
    article["article_key"] = "2512.24601"
    article["catalog_path"] = "arxiv/cs-ai/2512.24601"
    article_path.write_text(json.dumps(article), encoding="utf-8")

    result = _run_rebuild(
        catalog,
        index,
        selection,
        index.parent / "index-rebuild-report.json",
        index.parent / "index-rebuild-diagnostics.jsonl",
    )

    assert result.returncode == 1
    assert "duplicate" in result.stderr or "malformed_article_record" in result.stderr


def test_m025_article_catalog_verifier_rejects_unsafe_index_traversal(tmp_path: Path) -> None:
    catalog, index, selection = _copy_scaffold(tmp_path)
    payload = json.loads(index.read_text(encoding="utf-8"))
    payload["articles"][0]["article_path"] = "../outside/article.json"
    index.write_text(json.dumps(payload), encoding="utf-8")

    result = subprocess.run(
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
            "--check-safe-traversal",
        ],
        capture_output=True,
        check=False,
        text=True,
    )

    assert result.returncode == 1
    assert "unsafe catalog-relative path" in result.stderr or "non-canonical" in result.stderr
