from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

# pyrefly: ignore [missing-import]
from synthesize_m029_unified_readiness import (  # ty: ignore[unresolved-import]
    main as synthesize_main,  # noqa: E402  # ty:ignore[unresolved-import]
)

# pyrefly: ignore [missing-import]
from verify_m029_unified_readiness import (  # ty: ignore[unresolved-import]
    main as verify_main,  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
)


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _fixture_args(tmp_path: Path) -> tuple[list[str], list[str], Path]:
    corpus = tmp_path / "data" / "article_corpora" / "m029-unified-corpus-v1"
    runtime_event = corpus / "runtime-smoke" / "web_article-one.loader-events.jsonl"
    evidence = corpus / "evidence" / "article-one.evidence.json"
    replay_record = corpus / "replay" / "web_article-one.replay.json"
    runtime_event.parent.mkdir(parents=True, exist_ok=True)
    evidence.parent.mkdir(parents=True, exist_ok=True)
    replay_record.parent.mkdir(parents=True, exist_ok=True)
    runtime_event.write_text('{"event":"loaded"}\n', encoding="utf-8")
    evidence.write_text('{"metadata":"only"}\n', encoding="utf-8")
    replay_record.write_text('{"metadata":"only"}\n', encoding="utf-8")

    loaded_article = {
        "article_ref": "web/article-one",
        "article_key": "article-one",
        "identity_key": "web:article-one",
        "canonical_url": "https://example.test/article-one",
        "seed_url": "https://example.test/article-one",
        "source_code": "web",
        "source_strategy": "web_article_html",
        "provenance_sources": ["M025", "M027"],
        "provenance_url_count": 2,
    }
    zero_article = {
        "article_ref": "web/article-two",
        "article_key": "article-two",
        "identity_key": "web:article-two",
        "canonical_url": "https://example.test/article-two",
        "seed_url": "https://example.test/article-two",
        "source_code": "web",
        "source_strategy": "web_article_html",
        "provenance_sources": ["M030"],
        "provenance_url_count": 1,
    }
    selection = corpus / "selection.json"
    runtime_summary = corpus / "runtime-smoke-summary.json"
    replay_summary = corpus / "replay-summary.json"
    output_dir = corpus / "readiness"
    _write_json(
        selection,
        {
            "schema_version": "article-corpus-selection.v00.01",
            "selection_id": "m029-unified-corpus-v1",
            "articles": [loaded_article, zero_article],
        },
    )
    loaded_runtime = {
        **loaded_article,
        "status": "loaded",
        "diagnostic_code": "runtime_loader_loaded",
        "runtime_evidence_count": 1,
        "runtime_chunk_count": 3,
        "zero_chunk": False,
        "parser_ready_from_conversion": True,
        "runtime_event_log_path": runtime_event.relative_to(tmp_path).as_posix(),
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
    zero_runtime = {
        **zero_article,
        "status": "zero_chunk",
        "diagnostic_code": "runtime_loader_zero_chunk",
        "failure_reason": "no_parser_ready_converted_text",
        "runtime_evidence_count": 0,
        "runtime_chunk_count": 0,
        "zero_chunk": True,
        "parser_ready_from_conversion": False,
        "runtime_event_log_path": None,
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
            "runtime_summary_path": runtime_summary.relative_to(tmp_path).as_posix(),
            "results": [loaded_runtime, zero_runtime],
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
    loaded_replay = {
        **loaded_runtime,
        "status": "replay_loaded_verified",
        "diagnostic_code": "replay_loaded_verified",
        "evidence_path": evidence.relative_to(tmp_path).as_posix(),
        "replay_record_path": replay_record.relative_to(tmp_path).as_posix(),
    }
    zero_replay = {
        **zero_runtime,
        "status": "replay_zero_chunk_verified",
        "diagnostic_code": "replay_zero_chunk_verified",
        "evidence_path": None,
        "replay_record_path": None,
    }
    _write_json(
        replay_summary,
        {
            "schema_version": "m029-unified-replay.v1",
            "milestone_id": "M029-eb0ljz",
            "slice_id": "S05",
            "selection_id": "m029-unified-corpus-v1",
            "article_count": 2,
            "runtime_loaded_count": 1,
            "zero_chunk_count": 1,
            "runtime_evidence_count": 1,
            "replay_summary_path": replay_summary.relative_to(tmp_path).as_posix(),
            "results": [loaded_replay, zero_replay],
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
    run_args = [
        "synthesize_m029_unified_readiness.py",
        "--selection",
        str(selection),
        "--replay-summary",
        str(replay_summary),
        "--runtime-smoke-summary",
        str(runtime_summary),
        "--output-dir",
        str(output_dir),
    ]
    verify_args = [
        "verify_m029_unified_readiness.py",
        "--selection",
        str(selection),
        "--readiness-summary",
        str(corpus / "readiness-summary.json"),
        "--readiness-decision",
        str(corpus / "readiness-decision.json"),
        "--readiness-report",
        str(corpus / "readiness-report.md"),
        "--require-no-network",
        "--require-no-import-flags",
        "--check-dedupe-rule",
        "--check-provenance",
    ]
    return run_args, verify_args, corpus


def test_unified_readiness_writes_summary_decision_report(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _fixture_args(tmp_path)

    assert synthesize_main(run_args) == 0
    assert verify_main(verify_args) == 0

    summary = json.loads((corpus / "readiness-summary.json").read_text(encoding="utf-8"))
    decision = json.loads((corpus / "readiness-decision.json").read_text(encoding="utf-8"))
    report = (corpus / "readiness-report.md").read_text(encoding="utf-8")
    assert summary["article_count"] == 2
    assert summary["ready_count"] == 1
    assert summary["partial_count"] == 1
    assert summary["block_reason_counts"] == {"no_parser_ready_converted_text": 1}
    assert summary["provenance_source_counts"] == {"M025": 1, "M027": 1, "M030": 1}
    assert decision["decision"] == "partial_preprocessing_ready"
    assert "Dedupe and Provenance" in report
    assert "Final Counts and Block Reasons" in report
    assert "Graph import" in report


def test_unified_readiness_fails_closed_on_unsafe_replay_flag(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, _verify_args, corpus = _fixture_args(tmp_path)
    replay_path = corpus / "replay-summary.json"
    replay = json.loads(replay_path.read_text(encoding="utf-8"))
    replay["results"][0]["fail_closed_safety_flags"]["graph_import_allowed"] = True
    _write_json(replay_path, replay)

    assert synthesize_main(run_args) == 1
    stderr = capsys.readouterr().err
    assert "unsafe readiness input flags" in stderr
    assert "graph_import_allowed" in stderr


def test_unified_readiness_verifier_detects_count_drift(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _fixture_args(tmp_path)
    assert synthesize_main(run_args) == 0
    summary_path = corpus / "readiness-summary.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["ready_count"] = 2
    _write_json(summary_path, summary)

    assert verify_main(verify_args) == 1
    assert "ready_count_mismatch" in capsys.readouterr().err


def test_unified_readiness_verifier_writes_t02_summary(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    run_args, verify_args, corpus = _fixture_args(tmp_path)
    verify_summary_path = corpus / "readiness-verify-summary.json"
    assert synthesize_main(run_args) == 0

    assert verify_main([*verify_args, "--write-verify-summary", str(verify_summary_path)]) == 0

    verify_summary = json.loads(verify_summary_path.read_text(encoding="utf-8"))
    assert verify_summary["status"] == "passed"
    assert verify_summary["article_count"] == 2
    assert verify_summary["ready_count"] == 1
    assert verify_summary["partial_count"] == 1
    assert verify_summary["checks"]["dedupe_rule"] is True
    assert verify_summary["checks"]["provenance"] is True
    assert verify_summary["provenance_source_counts"] == {"M025": 1, "M027": 1, "M030": 1}
    assert verify_summary["diagnostics"] == []
