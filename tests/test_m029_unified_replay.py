from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from run_m029_unified_replay import main as run_main  # noqa: E402
from verify_m029_unified_replay import main as verify_main  # noqa: E402


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _fixture_args(tmp_path: Path) -> tuple[list[str], list[str], Path]:
    corpus = tmp_path / "data" / "article_corpora" / "m029-unified-corpus-v1"
    runtime_event = corpus / "runtime-smoke" / "web_article-one.loader-events.jsonl"
    converted_text = corpus / "conversion-quality" / "web_article-one" / "web_article_html.txt"
    runtime_event.parent.mkdir(parents=True, exist_ok=True)
    converted_text.parent.mkdir(parents=True, exist_ok=True)
    runtime_event.write_text('{"event":"loaded"}\n', encoding="utf-8")
    converted_text.write_text("metadata-only replay fixture body\n", encoding="utf-8")

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
    selection = corpus / "selection.json"
    runtime_summary = corpus / "runtime-smoke-summary.json"
    evidence_dir = corpus / "evidence"
    output_dir = corpus / "replay"
    _write_json(
        selection,
        {
            "schema_version": "article-corpus-selection.v00.01",
            "selection_id": "m029-unified-corpus-v1",
            "articles": [loaded_article, zero_article],
        },
    )
    loaded_row = {
        **loaded_article,
        "status": "loaded",
        "diagnostic_code": "runtime_loader_loaded",
        "runtime_evidence_count": 1,
        "runtime_chunk_count": 3,
        "zero_chunk": False,
        "parser_ready_from_conversion": True,
        "runtime_event_log_path": runtime_event.relative_to(tmp_path).as_posix(),
        "converted_text_path": converted_text.relative_to(tmp_path).as_posix(),
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "fail_closed_safety_flags": {
            "graph_import_allowed": False,
            "network_fetch_attempted": False,
        },
    }
    zero_row = {
        **zero_article,
        "status": "zero_chunk",
        "diagnostic_code": "runtime_loader_zero_chunk",
        "failure_reason": "no_parser_ready_converted_text",
        "runtime_evidence_count": 0,
        "runtime_chunk_count": 0,
        "zero_chunk": True,
        "parser_ready_from_conversion": False,
        "runtime_event_log_path": None,
        "converted_text_path": None,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "fail_closed_safety_flags": {
            "graph_import_allowed": False,
            "network_fetch_attempted": False,
        },
    }
    _write_json(
        runtime_summary,
        {
            "schema_version": "m029-runtime-smoke.v1",
            "milestone_id": "M029-eb0ljz",
            "slice_id": "S04",
            "selection_id": "m029-unified-corpus-v1",
            "article_count": 2,
            "runtime_loaded_count": 1,
            "zero_chunk_count": 1,
            "runtime_evidence_count": 1,
            "results": [loaded_row, zero_row],
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
            "graph_import_allowed": False,
            "fail_closed_safety_flags": {
                "graph_import_allowed": False,
                "network_fetch_attempted": False,
            },
        },
    )
    for row in [loaded_row, zero_row]:
        evidence = {
            **row,
            "schema_version": "m029-loader-evidence.v1",
            "milestone_id": "M029-eb0ljz",
            "slice_id": "S04",
            "selection_id": "m029-unified-corpus-v1",
            "evidence_path": (evidence_dir / f"{row['article_key']}.evidence.json")
            .relative_to(tmp_path)
            .as_posix(),
        }
        _write_json(evidence_dir / f"{row['article_key']}.evidence.json", evidence)

    run_args = [
        "run_m029_unified_replay.py",
        "--selection",
        str(selection),
        "--runtime-smoke-summary",
        str(runtime_summary),
        "--evidence-dir",
        str(evidence_dir),
        "--output-dir",
        str(output_dir),
    ]
    verify_args = [
        "verify_m029_unified_replay.py",
        "--selection",
        str(selection),
        "--replay-summary",
        str(corpus / "replay-summary.json"),
        "--replay-diagnostics",
        str(corpus / "replay-diagnostics.jsonl"),
        "--replay-report",
        str(corpus / "replay-report.md"),
        "--compare-runtime-smoke",
        str(runtime_summary),
        "--require-no-network",
        "--require-no-import-flags",
    ]
    return run_args, verify_args, corpus


def test_unified_replay_writes_summary_diagnostics_report(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _fixture_args(tmp_path)

    assert run_main(run_args) == 0
    assert verify_main(verify_args) == 0

    summary = json.loads((corpus / "replay-summary.json").read_text(encoding="utf-8"))
    diagnostics = [
        json.loads(line)
        for line in (corpus / "replay-diagnostics.jsonl").read_text(encoding="utf-8").splitlines()
    ]
    report = (corpus / "replay-report.md").read_text(encoding="utf-8")
    assert summary["article_count"] == 2
    assert summary["runtime_loaded_count"] == 1
    assert summary["zero_chunk_count"] == 1
    assert summary["runtime_evidence_count"] == 1
    assert summary["network_fetch_attempted"] is False
    assert len(diagnostics) == 2
    assert "## Article Coverage" in report
    assert "## Evidence Surfaces" in report
    assert "## Safety Flags" in report
    assert len(list((corpus / "replay").glob("*.replay.json"))) == 2


def test_unified_replay_fails_when_evidence_record_missing(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, _verify_args, corpus = _fixture_args(tmp_path)
    (corpus / "evidence" / "article-two.evidence.json").unlink()

    assert run_main(run_args) == 1
    assert "missing evidence record" in capsys.readouterr().err


def test_unified_replay_fails_closed_on_unsafe_evidence_flag(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, _verify_args, corpus = _fixture_args(tmp_path)
    evidence_path = corpus / "evidence" / "article-one.evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    evidence["fail_closed_safety_flags"]["graph_import_allowed"] = True
    _write_json(evidence_path, evidence)

    assert run_main(run_args) == 1
    stderr = capsys.readouterr().err
    assert "unsafe flags" in stderr
    assert "graph_import_allowed" in stderr


def test_unified_replay_verifier_detects_runtime_count_drift(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _fixture_args(tmp_path)
    assert run_main(run_args) == 0
    summary_path = corpus / "replay-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["results"][0]["runtime_evidence_count"] = 0
    summary["runtime_evidence_count"] = 0
    _write_json(summary_path, summary)

    assert verify_main(verify_args) == 1
    assert "runtime_replay_count_mismatch" in capsys.readouterr().err
