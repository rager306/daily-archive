from __future__ import annotations

import importlib.util
import json
import sys
from argparse import Namespace
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = (
    Path(__file__).parents[1] / "scripts" / "verify_m027_provenance_and_riskratchet_gate.py"
)
spec = importlib.util.spec_from_file_location(
    "verify_m027_provenance_and_riskratchet_gate", MODULE_PATH
)
assert spec is not None and spec.loader is not None
gate = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = gate
spec.loader.exec_module(gate)


def _sha(path: Path) -> str:
    return gate.sha256_file(path)


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8"
    )


def _artifact_row(path: Path, root: Path, role: str) -> dict[str, Any]:
    return {
        "role": role,
        "path": path.relative_to(root).as_posix(),
        "exists": True,
        "sha256": _sha(path),
        "byte_size": path.stat().st_size,
    }


def _fixture(tmp_path: Path) -> Namespace:
    root = tmp_path
    corpus = root / "data" / "article_corpora" / "m027-mixed-source-corpus-v1"
    replay_dir = corpus / "end-to-end-mixed-replay"
    replay_artifact = replay_dir / "article_one" / "replay.json"
    _write_json(
        replay_artifact,
        {
            "schema_version": "m027-end-to-end-mixed-replay-artifact.v1",
            "article_ref": "article/one",
            "variant_count": 1,
            "variants": [
                {"article_ref": "article/one", "variant_id": "one:source:pdf", "parser_ready": True}
            ],
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
        },
    )
    diagnostics_path = corpus / "end-to-end-mixed-replay-diagnostics.jsonl"
    events_path = corpus / "end-to-end-mixed-replay-events.jsonl"
    report_path = corpus / "end-to-end-mixed-replay-report.md"
    decision_path = corpus / "end-to-end-mixed-replay-readiness-decision.json"
    verification_path = corpus / "end-to-end-mixed-replay-verification.json"
    _write_jsonl(
        diagnostics_path,
        [
            {
                "schema_version": "m027-end-to-end-mixed-replay-diagnostic.v1",
                "diagnostic_code": "end_to_end_boundaries_completed",
                "network_fetch_attempted": False,
            }
        ],
    )
    _write_jsonl(
        events_path,
        [
            {
                "schema_version": "m027-end-to-end-mixed-replay-diagnostic.v1",
                "event_type": "replay_completed",
                "network_fetch_attempted": False,
            }
        ],
    )
    report_path.write_text("# S05 report\n\nredacted metadata only\n", encoding="utf-8")
    _write_json(
        decision_path,
        {
            "schema_version": "m027-end-to-end-mixed-replay-readiness-decision.v1",
            "ready_for_import": False,
            "network_fetch_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
        },
    )
    _write_json(
        verification_path,
        {
            "schema_version": "m027-end-to-end-mixed-replay-verifier.v1",
            "status": "passed",
            "verification_result": "passed",
            "network_fetch_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
        },
    )
    summary_path = corpus / "end-to-end-mixed-replay-summary.json"
    _write_json(
        summary_path,
        {
            "schema_version": "m027-end-to-end-mixed-replay.v1",
            "milestone_id": "M027-aakeky",
            "slice_id": "S05",
            "selection_id": "m027-mixed-source-corpus-v1",
            "status": "completed",
            "article_count": 1,
            "variant_count": 1,
            "output_artifacts": [],
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "trusted_kg_import_allowed": False,
            "ladybugdb_written": False,
        },
    )
    summary_payload = json.loads(summary_path.read_text(encoding="utf-8"))
    summary_payload["output_artifacts"] = [
        _artifact_row(summary_path, root, "summary"),
        _artifact_row(diagnostics_path, root, "diagnostics"),
        _artifact_row(events_path, root, "events"),
        _artifact_row(report_path, root, "report"),
        _artifact_row(decision_path, root, "readiness_decision"),
        _artifact_row(replay_artifact, root, "per_article_replay"),
    ]
    _write_json(summary_path, summary_payload)
    gate_dir = corpus / "provenance-riskratchet-gate"
    return Namespace(
        root=root,
        s05_summary=summary_path,
        s05_diagnostics=diagnostics_path,
        s05_events=events_path,
        s05_readiness_decision=decision_path,
        s05_verification=verification_path,
        output_dir=gate_dir,
        summary_output=gate_dir / "provenance-riskratchet-gate-summary.json",
        diagnostics_output=gate_dir / "provenance-riskratchet-gate-diagnostics.jsonl",
        report_output=gate_dir / "provenance-riskratchet-gate-report.md",
        maintainability_json=gate_dir / "maintainability-diagnostic.json",
        maintainability_report=gate_dir / "maintainability-diagnostic.md",
        validate_only=False,
    )


def _fake_risk_report(
    args: Namespace,
    monkeypatch: pytest.MonkeyPatch,
    *,
    blocking: bool = False,
    pass_fail_affected: bool = False,
) -> None:
    def fake_run_quality_gate(*, paths, output_dir, baseline_path=None, base_ref="HEAD"):
        output_path = Path(output_dir)
        output_path.mkdir(parents=True, exist_ok=True)
        report = {
            "schema_version": "test-riskratchet.v1",
            "status": "diagnostic_complete",
            "diagnostic_only": True,
            "blocking": blocking,
            "pass_fail_affected": pass_fail_affected,
            "tool_status": "ok",
            "tool_error": None,
            "summary": {
                "total_functions": 3,
                "max_score": 12.0,
                "average_score": 4.0,
                "by_severity": {"low": 2, "medium": 1, "high": 0, "critical": 0},
            },
            "riskratchet": {"blocking": False, "functions": []},
            "quality_gate": {
                "diagnostic_only": True,
                "blocking": blocking,
                "pass_fail_affected": pass_fail_affected,
                "touched_module_count": len(paths),
                "touched_modules": [str(path) for path in paths],
                "json_report": str(args.maintainability_json),
                "human_report": str(args.maintainability_report),
            },
        }
        _write_json(args.maintainability_json, report)
        args.maintainability_report.write_text(
            "# Local Maintainability Diagnostic\n\nDiagnostic-only and non-blocking.\n",
            encoding="utf-8",
        )
        return report

    monkeypatch.setattr(gate.run_quality_gate, "run_quality_gate", fake_run_quality_gate)


def _codes(rows: list[dict[str, Any]]) -> set[str]:
    return {str(row.get("diagnostic_code")) for row in rows}


def test_gate_generates_happy_path_and_self_hash_exclusion(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    args = _fixture(tmp_path)
    _fake_risk_report(args, monkeypatch)

    summary, diagnostics = gate.generate(args)

    assert diagnostics == []
    assert summary["status"] == "passed"
    assert summary["schema_version"] == "m027-provenance-riskratchet-gate.v1"
    assert summary["provenance"]["self_hash_excluded"] is True
    assert "self-referential" in summary["provenance"]["self_hash_excluded_reason"]
    assert summary["provenance"]["command"]
    assert summary["provenance"]["cwd"]
    assert summary["provenance"]["exit_code"] == 0
    assert summary["safety"]["network_fetch_attempted"] is False
    assert summary["safety"]["graph_import_allowed"] is False
    assert summary["riskratchet"]["blocking"] is False
    assert summary["riskratchet"]["pass_fail_affected"] is False
    assert args.summary_output.exists()
    assert args.diagnostics_output.exists()
    assert args.report_output.exists()
    report_text = args.report_output.read_text(encoding="utf-8")
    assert "validate-only, local-only audit artifact" in report_text
    assert "diagnostic-only and non-blocking" in report_text
    assert "not an import/readiness approval" in report_text
    assert args.maintainability_json.exists()
    assert args.maintainability_report.exists()
    roles = {row["role"] for row in summary["provenance"]["output_artifacts"]}
    assert {
        "summary",
        "diagnostics",
        "report",
        "maintainability_json",
        "maintainability_report",
    } <= roles


def test_validate_only_reads_existing_outputs_without_rerunning_riskratchet(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    args = _fixture(tmp_path)
    _fake_risk_report(args, monkeypatch)
    gate.generate(args)
    mtime = args.summary_output.stat().st_mtime_ns

    def forbidden_run_quality_gate(*args: Any, **kwargs: Any) -> dict[str, Any]:
        raise AssertionError("validate-only must not rerun riskratchet")

    monkeypatch.setattr(gate.run_quality_gate, "run_quality_gate", forbidden_run_quality_gate)

    summary, diagnostics = gate.validate_existing(args)

    assert diagnostics == []
    assert summary["status"] == "passed"
    assert args.summary_output.stat().st_mtime_ns == mtime


def test_validate_only_reports_missing_and_malformed_artifacts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    args = _fixture(tmp_path)
    _fake_risk_report(args, monkeypatch)
    gate.generate(args)
    args.diagnostics_output.unlink()
    args.maintainability_json.write_text("{not-json}\n", encoding="utf-8")

    _, diagnostics = gate.validate_existing(args)

    codes = _codes(diagnostics)
    assert "missing_jsonl_artifact" in codes
    assert "malformed_json_artifact" in codes


def test_gate_rejects_unsafe_flags_redaction_riskratchet_and_paths(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    args = _fixture(tmp_path)
    _fake_risk_report(args, monkeypatch, blocking=True, pass_fail_affected=True)
    summary_payload = json.loads(args.s05_summary.read_text(encoding="utf-8"))
    summary_payload["graph_import_allowed"] = True
    summary_payload["output_artifacts"][1]["sha256"] = "0" * 64
    summary_payload["output_artifacts"][2]["path"] = "https://example.invalid/events.jsonl"
    _write_json(args.s05_summary, summary_payload)
    args.s05_readiness_decision.write_text(
        json.dumps(
            {
                "ready_for_import": False,
                "safe_note": "RAW_PDF_SECRET <html",
                "graph_import_allowed": False,
            }
        )
        + "\n",
        encoding="utf-8",
    )

    summary, diagnostics = gate.generate(args)

    codes = _codes(diagnostics)
    assert summary["status"] == "failed"
    assert "unsafe_safety_flag_true" in codes
    assert "metadata_payload_snippet_leakage" in codes
    assert "riskratchet_blocking_true" in codes
    assert "riskratchet_pass_fail_affected_true" in codes
    assert "s05_output_artifact_sha256_mismatch" in codes
    assert any(code.startswith("unsafe_s05_output_artifact_path") for code in codes)


def test_validate_only_rejects_unsafe_summary_flag_and_stale_output_hash(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    args = _fixture(tmp_path)
    _fake_risk_report(args, monkeypatch)
    gate.generate(args)
    payload = json.loads(args.summary_output.read_text(encoding="utf-8"))
    payload["safety"]["graph_import_allowed"] = True
    for row in payload["provenance"]["output_artifacts"]:
        if row["role"] == "report":
            row["sha256"] = "0" * 64
    _write_json(args.summary_output, payload)

    _, diagnostics = gate.validate_existing(args)

    codes = _codes(diagnostics)
    assert "unsafe_safety_flag_true" in codes
    assert "output_artifact_sha256_mismatch" in codes
