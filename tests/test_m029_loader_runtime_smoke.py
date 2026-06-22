from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from run_m029_unified_loader_runtime_smoke import main as run_main  # noqa: E402
from verify_m029_unified_loader_runtime_smoke import main as verify_main  # noqa: E402


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _runtime_args(tmp_path: Path) -> tuple[list[str], list[str], Path]:
    corpus = tmp_path / "data" / "article_corpora" / "m029-unified-corpus-v1"
    converted = corpus / "conversion-quality" / "article-one" / "web_article_html.txt"
    converted.parent.mkdir(parents=True, exist_ok=True)
    converted.write_text(
        "This converted article body is parser ready.\n"
        "It has substantive local text for the runtime smoke loader.\n",
        encoding="utf-8",
    )
    selection = corpus / "selection.json"
    conversion_summary = corpus / "conversion-quality-summary.json"
    source_summary = corpus / "source-acquisition-summary.json"
    output_dir = corpus / "runtime-smoke"
    loaded_article = {
        "article_ref": "web/article-one",
        "article_key": "article-one",
        "identity_key": "web:article-one",
        "canonical_url": "https://example.test/article-one",
        "seed_url": "https://example.test/article-one",
        "source_code": "web",
        "source_strategy": "web_article_html",
    }
    zero_article = {
        "article_ref": "web/article-two",
        "article_key": "article-two",
        "identity_key": "web:article-two",
        "canonical_url": "https://example.test/article-two",
        "seed_url": "https://example.test/article-two",
        "source_code": "web",
        "source_strategy": "web_article_html",
    }
    _write_json(
        selection,
        {
            "schema_version": "article-corpus-selection.v00.01",
            "selection_id": "m029-unified-corpus-v1",
            "articles": [loaded_article, zero_article],
        },
    )
    converted_rel = converted.relative_to(tmp_path).as_posix()
    _write_json(
        conversion_summary,
        {
            "schema_version": "m029-conversion-quality.v1",
            "selection_id": "m029-unified-corpus-v1",
            "article_count": 2,
            "results": [
                {
                    **loaded_article,
                    "variant_id": "article-one:source:web-html",
                    "source_role": "web_article_html",
                    "status": "converted",
                    "diagnostic_code": "parser_ready_converted_text",
                    "parser_ready": True,
                    "converted_text_path": converted_rel,
                    "converted_text_sha256": "fixture-sha",
                    "converted_text_byte_size": converted.stat().st_size,
                    "fail_closed_safety_flags": {"graph_import_allowed": False},
                    "graph_import_allowed": False,
                    "production_import_attempted": False,
                    "ladybugdb_written": False,
                    "trusted_kg_import_allowed": False,
                    "network_fetch_attempted": False,
                },
                {
                    **zero_article,
                    "variant_id": "article-two:source:web-html",
                    "source_role": "web_article_html",
                    "status": "metadata_only",
                    "diagnostic_code": "metadata_only_no_substantive_body",
                    "parser_ready": False,
                    "converted_text_path": None,
                    "converted_text_sha256": None,
                    "converted_text_byte_size": 0,
                    "fail_closed_safety_flags": {"graph_import_allowed": False},
                    "graph_import_allowed": False,
                    "production_import_attempted": False,
                    "ladybugdb_written": False,
                    "trusted_kg_import_allowed": False,
                    "network_fetch_attempted": False,
                },
            ],
        },
    )
    _write_json(
        source_summary,
        {
            "schema_version": "m029-source-acquisition.v1",
            "selection_id": "m029-unified-corpus-v1",
            "article_count": 2,
            "results": [
                {"variant_id": "article-one:source:web-html", "status": "captured"},
                {"variant_id": "article-two:source:web-html", "status": "captured"},
            ],
        },
    )
    run_args = [
        "run_m029_unified_loader_runtime_smoke.py",
        "--selection",
        str(selection),
        "--conversion-summary",
        str(conversion_summary),
        "--source-summary",
        str(source_summary),
        "--output-dir",
        str(output_dir),
    ]
    verify_args = [
        "verify_m029_unified_loader_runtime_smoke.py",
        "--selection",
        str(selection),
        "--conversion-summary",
        str(conversion_summary),
        "--runtime-smoke-summary",
        str(corpus / "runtime-smoke-summary.json"),
        "--runtime-smoke-diagnostics",
        str(corpus / "runtime-smoke-diagnostics.jsonl"),
        "--runtime-smoke-report",
        str(corpus / "runtime-smoke-report.md"),
        "--require-no-network",
        "--require-no-import-flags",
        "--check-selection-count",
        "2",
        "--check-article-identity",
        "--check-source-strategy-mapping",
        "--check-parser-ready-alignment",
    ]
    return run_args, verify_args, corpus


def _with_evidence_args(verify_args: list[str], corpus: Path) -> list[str]:
    return [
        *verify_args,
        "--evidence-dir",
        str(corpus / "evidence"),
        "--write-evidence-summary",
        str(corpus / "evidence-summary.json"),
        "--write-evidence-diagnostics",
        str(corpus / "evidence-diagnostics.jsonl"),
        "--check-evidence-counts",
        "--check-zero-chunk-outcomes",
    ]


def test_runtime_smoke_writes_loaded_and_zero_chunk_artifacts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _runtime_args(tmp_path)

    assert run_main(run_args) == 0
    assert verify_main(verify_args) == 0

    summary = json.loads((corpus / "runtime-smoke-summary.json").read_text(encoding="utf-8"))
    assert summary["article_count"] == 2
    assert summary["runtime_loaded_count"] == 1
    assert summary["zero_chunk_count"] == 1
    assert summary["runtime_evidence_count"] == 1
    rows = {row["article_ref"]: row for row in summary["results"]}
    assert rows["web/article-one"]["status"] == "loaded"
    assert rows["web/article-one"]["runtime_chunk_count"] >= 1
    assert rows["web/article-two"]["status"] == "zero_chunk"
    assert rows["web/article-two"]["failure_reason"] == "no_parser_ready_converted_text"
    assert summary["network_fetch_attempted"] is False
    assert summary["production_import_attempted"] is False
    assert summary["ladybugdb_written"] is False


def test_runtime_verifier_fails_closed_on_identity_mismatch(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _runtime_args(tmp_path)
    assert run_main(run_args) == 0
    summary_path = corpus / "runtime-smoke-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["results"][0]["identity_key"] = "drifted:identity"
    _write_json(summary_path, summary)

    assert verify_main(verify_args) == 1
    assert "article_identity_mismatch" in capsys.readouterr().err


def test_runtime_verifier_fails_closed_on_import_flag(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _runtime_args(tmp_path)
    assert run_main(run_args) == 0
    summary_path = corpus / "runtime-smoke-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["results"][0]["fail_closed_safety_flags"]["graph_import_allowed"] = True
    _write_json(summary_path, summary)

    assert verify_main(verify_args) == 1
    stderr = capsys.readouterr().err
    assert "unsafe_runtime_flag_true" in stderr
    assert "graph_import_allowed" in stderr


def test_runtime_verifier_fails_when_parser_ready_article_is_zero_chunk(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _runtime_args(tmp_path)
    assert run_main(run_args) == 0
    summary_path = corpus / "runtime-smoke-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["results"][0]["status"] = "zero_chunk"
    summary["results"][0]["runtime_evidence_count"] = 0
    summary["runtime_loaded_count"] = 0
    summary["zero_chunk_count"] = 2
    summary["runtime_evidence_count"] = 0
    _write_json(summary_path, summary)

    assert verify_main(verify_args) == 1
    assert "parser_ready_article_not_loaded" in capsys.readouterr().err


def test_runtime_verifier_writes_loader_evidence_bundle(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _runtime_args(tmp_path)
    assert run_main(run_args) == 0

    assert verify_main(_with_evidence_args(verify_args, corpus)) == 0

    evidence_summary = json.loads((corpus / "evidence-summary.json").read_text(encoding="utf-8"))
    diagnostics = [
        json.loads(line)
        for line in (corpus / "evidence-diagnostics.jsonl").read_text(encoding="utf-8").splitlines()
    ]
    evidence_files = sorted((corpus / "evidence").glob("*.evidence.json"))
    assert evidence_summary["article_count"] == 2
    assert evidence_summary["evidence_record_count"] == 2
    assert evidence_summary["runtime_evidence_count"] == 1
    assert evidence_summary["zero_chunk_count"] == 1
    assert len(diagnostics) == 2
    assert len(evidence_files) == 2
    zero_record = json.loads(
        (corpus / "evidence" / "web_article-two.evidence.json").read_text(encoding="utf-8")
    )
    assert zero_record["zero_chunk"] is True
    assert zero_record["runtime_evidence_count"] == 0
    assert zero_record["failure_reason"] == "no_parser_ready_converted_text"
    assert zero_record["source_strategy"] == "web_article_html"
    assert zero_record["network_fetch_attempted"] is False
    assert zero_record["production_import_attempted"] is False
    assert zero_record["ladybugdb_written"] is False


def test_evidence_check_fails_when_zero_chunk_diagnostic_is_missing(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _runtime_args(tmp_path)
    assert run_main(run_args) == 0
    summary_path = corpus / "runtime-smoke-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["results"][1]["failure_reason"] = None
    _write_json(summary_path, summary)

    assert verify_main(_with_evidence_args(verify_args, corpus)) == 1
    assert "zero_chunk_missing_failure_diagnostic" in capsys.readouterr().err
