from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path
from typing import Any

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "synthesize_m027_pipeline_readiness.py"
spec = importlib.util.spec_from_file_location("synthesize_m027_pipeline_readiness", MODULE_PATH)
assert spec is not None and spec.loader is not None
synthesis = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = synthesis
spec.loader.exec_module(synthesis)


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def test_load_inputs_reports_missing_json_artifact(tmp_path: Path, monkeypatch: Any) -> None:
    missing = tmp_path / "missing-summary.json"
    monkeypatch.setattr(synthesis, "INPUT_ARTIFACTS", (("s01_catalog_summary", missing, "missing fixture"),))

    payloads, jsonl_payloads, diagnostics, input_rows = synthesis.load_inputs(tmp_path)

    assert payloads == {}
    assert jsonl_payloads == {}
    assert input_rows[0]["exists"] is False
    assert {row["diagnostic_code"] for row in diagnostics} == {"missing_json_artifact"}
    assert diagnostics[0]["failure_source_class"] == "filesystem"


def test_url_like_artifact_reference_is_rejected(tmp_path: Path, monkeypatch: Any) -> None:
    summary = tmp_path / "summary.json"
    _write_json(
        summary,
        {
            "schema_version": "article-corpus-run-summary.v00.01",
            "milestone_id": synthesis.MILESTONE_ID,
            "slice_id": "S01",
            "selection_id": synthesis.SELECTION_ID,
            "output_summary_path": "https://example.invalid/summary.json",
            **synthesis.FALSE_SAFETY_FLAGS,
        },
    )
    monkeypatch.setattr(synthesis, "INPUT_ARTIFACTS", (("s01_catalog_summary", summary, "fixture"),))

    _payloads, _jsonl_payloads, diagnostics, _input_rows = synthesis.load_inputs(tmp_path)

    codes = {row["diagnostic_code"] for row in diagnostics}
    assert any(code.startswith("unsafe_artifact_reference:url_not_allowed") for code in codes)
    assert any(row["failure_source_class"] == "path_safety" for row in diagnostics)


def test_readiness_claim_creep_is_blocking(tmp_path: Path, monkeypatch: Any) -> None:
    decision = tmp_path / "decision.json"
    _write_json(
        decision,
        {
            "schema_version": "m027-end-to-end-mixed-replay-readiness-decision.v1",
            "milestone_id": synthesis.MILESTONE_ID,
            "slice_id": "S05",
            "selection_id": synthesis.SELECTION_ID,
            "decision": "import_ready",
            "ready_for_import": True,
            **synthesis.FALSE_SAFETY_FLAGS,
        },
    )
    monkeypatch.setattr(synthesis, "INPUT_ARTIFACTS", (("s05_readiness_decision", decision, "fixture"),))

    _payloads, _jsonl_payloads, diagnostics, _input_rows = synthesis.load_inputs(tmp_path)

    codes = {row["diagnostic_code"] for row in diagnostics}
    assert "unsafe_safety_or_readiness_flag_true" in codes
    assert "readiness_decision_claim_creep" in codes
    assert any(row["failure_source_class"] == "claim_boundary" for row in diagnostics)


def test_declared_artifact_hash_mismatch_is_reported(tmp_path: Path) -> None:
    artifact = tmp_path / "artifact.json"
    artifact.write_text('{"ok": true}\n', encoding="utf-8")
    where = tmp_path / "summary.json"
    _write_json(where, {"output_artifacts": [{"role": "diagnostics", "path": "artifact.json", "sha256": "0" * 64, "byte_size": artifact.stat().st_size}]})

    findings = synthesis.validate_declared_artifact_rows(
        [{"role": "diagnostics", "path": "artifact.json", "sha256": "0" * 64, "byte_size": artifact.stat().st_size}],
        where=where,
        root=tmp_path,
        json_path="$.output_artifacts",
    )

    assert {row["diagnostic_code"] for row in findings} == {"declared_artifact_sha256_mismatch"}
    assert findings[0]["failure_source_class"] == "provenance_hash"
