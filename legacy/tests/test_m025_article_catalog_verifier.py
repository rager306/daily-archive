from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

from scripts.verify_m025_article_catalog import (
    CATALOG_RECORD_DIR,
    FORBIDDEN_TRUE_FLAGS,
    article_ref_from_path,
    check_safety_flags,
    normalize_posix_path,
    safe_catalog_path,
)

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


def test_m186_catalog_path_helpers_are_fail_closed(tmp_path: Path) -> None:
    catalog = tmp_path / "catalog.json"
    catalog.write_text("{}", encoding="utf-8")
    article_path = f"{CATALOG_RECORD_DIR}/arxiv/cs-ai/1234.56789/article.json"

    assert normalize_posix_path(r"article_catalog\\arxiv\\cs-ai\\1234.56789\\article.json") == article_path
    assert safe_catalog_path(catalog, article_path) == (tmp_path / article_path).resolve()
    assert article_ref_from_path(article_path) == "arxiv/cs-ai/1234.56789"

    for unsafe in ("/article.json", "../article.json", f"{CATALOG_RECORD_DIR}/../secret/article.json"):
        try:
            safe_catalog_path(catalog, unsafe)
        except ValueError:
            pass
        else:  # pragma: no cover - assertion message is the contract
            raise AssertionError(f"unsafe catalog path accepted: {unsafe!r}")

    for noncanonical in (
        "arxiv/cs-ai/1234.56789/article.json",
        f"{CATALOG_RECORD_DIR}/arxiv/cs-ai/1234.56789/source.pdf",
    ):
        try:
            article_ref_from_path(noncanonical)
        except ValueError:
            pass
        else:  # pragma: no cover - assertion message is the contract
            raise AssertionError(f"non-canonical article path accepted: {noncanonical!r}")


def test_m186_safety_flags_reject_forbidden_true_values() -> None:
    errors: list[str] = []
    safe_payload = dict.fromkeys(FORBIDDEN_TRUE_FLAGS, False)
    check_safety_flags(errors, "catalog", safe_payload)
    assert errors == []

    unsafe_key = next(iter(FORBIDDEN_TRUE_FLAGS))
    check_safety_flags(errors, "catalog", {"nested": [{unsafe_key: True}]})
    assert errors == [f"catalog.nested[0].{unsafe_key} must be false; got True"]


def _run_catalog_report(
    catalog: Path,
    index: Path,
    selection: Path,
    summary: Path,
    diagnostics: Path,
    report: Path,
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
            "--validate-only",
            "--require-index",
            "--check-index-idempotent",
            "--write-summary",
            str(summary),
            "--write-diagnostics",
            str(diagnostics),
            "--write-report",
            str(report),
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


def test_m025_article_catalog_rebuild_writes_idempotent_report_and_diagnostics(
    tmp_path: Path,
) -> None:
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


def test_m025_article_catalog_verifier_writes_catalog_readiness_outputs(tmp_path: Path) -> None:
    catalog, index, selection = _copy_scaffold(tmp_path)
    summary_path = selection.parent / "run-summary.json"
    diagnostics_path = selection.parent / "diagnostics.jsonl"
    report_path = selection.parent / "catalog-report.md"

    result = _run_catalog_report(
        catalog, index, selection, summary_path, diagnostics_path, report_path
    )

    assert result.returncode == 0, result.stderr
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["schema_version"] == "article-corpus-run-summary.v00.01"
    assert summary["article_count"] == 5
    assert summary["readiness"]["blocked_article_count"] == 5
    assert summary["index"]["idempotent"] is True
    assert summary["network"]["network_fetch_attempted_during_validation"] is False
    diagnostics = [
        json.loads(line) for line in diagnostics_path.read_text(encoding="utf-8").splitlines()
    ]
    assert {row["code"] for row in diagnostics} >= {
        "index_readiness",
        "article_readiness",
        "source_variant_readiness",
    }
    report = report_path.read_text(encoding="utf-8")
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report
    assert "Ready for S02 parser/chunking baseline: False" in report


def test_m025_article_catalog_rebuild_rejects_duplicate_lookup_key(tmp_path: Path) -> None:
    catalog, index, selection = _copy_scaffold(tmp_path)
    article_path = (
        index.parent / "article_catalog" / "arxiv" / "cs-ai" / "2605.28617v1" / "article.json"
    )
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
