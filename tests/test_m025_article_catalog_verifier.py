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
