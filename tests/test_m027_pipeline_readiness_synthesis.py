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


def _write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")


def _valid_s07_outputs(root: Path) -> tuple[Path, Path, Path, dict[str, Any]]:
    out_dir = root / "out"
    summary_path = out_dir / "summary.json"
    diagnostics_path = out_dir / "diagnostics.jsonl"
    report_path = out_dir / "report.md"
    _write_jsonl(diagnostics_path, [])
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        "\n".join(
            [
                "# M027 Pipeline Readiness Synthesis",
                "## Ready now",
                "- Local preprocessing evidence is present.",
                "## Ready with blockers/conditions",
                "- One parser-ready variant still produces zero chunks.",
                "## Not ready",
                "- Not ready for graph import, trusted facts, import-ready chunks, scientific KG quality, production writes, DSPy/RLM optimization, unattended scaling, or LadybugDB writes.",
                "## Requirement coverage",
                "## Health, failure, and recovery",
                "## Next-cycle recommendations",
                "",
            ]
        ),
        encoding="utf-8",
    )
    summary = {
        "schema_version": synthesis.SCHEMA_VERSION,
        "slice_id": synthesis.SLICE_ID,
        "health": {"diagnostic_count": 0},
        "functional_readiness": {
            "ready_with_blockers_conditions": ["One parser-ready variant still produces zero chunks."],
            "not_ready": ["Not ready for graph import, trusted facts, import-ready chunks, scientific KG quality, production writes, DSPy/RLM optimization, unattended scaling, or LadybugDB writes."],
        },
        "provenance": {"self_hash_excluded": True, "output_artifacts": []},
        **synthesis.FALSE_SAFETY_FLAGS,
    }
    _write_json(summary_path, summary)
    summary["provenance"]["output_artifacts"] = [
        {"role": "summary", "path": "out/summary.json", "sha256": None, "byte_size": summary_path.stat().st_size},
        synthesis.artifact_row(diagnostics_path, role="diagnostics", description="diagnostics", root=root),
        synthesis.artifact_row(report_path, role="report", description="report", root=root),
    ]
    _write_json(summary_path, summary)
    return summary_path, diagnostics_path, report_path, summary


def test_real_artifact_validate_only_path_passes() -> None:
    assert synthesis.main(["--validate-only"]) == 0


def test_malformed_json_artifact_reports_stable_code(tmp_path: Path) -> None:
    malformed = tmp_path / "bad.json"
    malformed.write_text("{not json", encoding="utf-8")

    payload, diagnostics = synthesis.load_json(malformed)

    assert payload is None
    assert diagnostics[0]["diagnostic_code"] == "malformed_json_artifact"
    assert diagnostics[0]["failure_source_class"] == "json_contract"


def test_malformed_jsonl_artifact_reports_stable_code(tmp_path: Path) -> None:
    malformed = tmp_path / "bad.jsonl"
    malformed.write_text('{"ok": true}\n[1, 2]\n{not json}\n', encoding="utf-8")

    rows, diagnostics = synthesis.load_jsonl(malformed)

    assert rows == [{"ok": True}]
    assert [row["diagnostic_code"] for row in diagnostics] == ["malformed_jsonl_artifact", "malformed_jsonl_artifact"]
    assert {row["json_path"] for row in diagnostics} == {"$[2]", "$[3]"}


def test_unsafe_boolean_flags_are_rejected(tmp_path: Path) -> None:
    payload = {"graph_import_allowed": True, "ladybugdb_written": True, "trusted_fact_claim": True}

    diagnostics = synthesis.false_flag_diagnostics(payload, where=tmp_path / "summary.json")

    assert [row["diagnostic_code"] for row in diagnostics] == [
        "unsafe_safety_or_readiness_flag_true",
        "unsafe_safety_or_readiness_flag_true",
        "unsafe_safety_or_readiness_flag_true",
    ]
    assert {row["failure_source_class"] for row in diagnostics} == {"safety_flags", "claim_boundary"}


def test_riskratchet_blocking_and_pass_fail_are_rejected(tmp_path: Path) -> None:
    where = tmp_path / "riskratchet.json"
    payload = {
        "schema_version": "m027-provenance-riskratchet-gate.v1",
        "slice_id": "S06",
        "selection_id": synthesis.SELECTION_ID,
        "riskratchet": {"diagnostic_only": True, "blocking": True, "pass_fail_affected": True},
    }

    diagnostics = synthesis.validate_contract("s06_riskratchet_summary", payload, where=where)

    assert {row["diagnostic_code"] for row in diagnostics} == {"riskratchet_summary_blocking_or_not_diagnostic_only"}
    assert diagnostics[0]["failure_source_class"] == "riskratchet"
    assert diagnostics[0]["json_path"] == "$.riskratchet"


def test_maintainability_blocking_and_pass_fail_are_rejected(tmp_path: Path) -> None:
    where = tmp_path / "maintainability.json"
    payload = {"diagnostic_only": True, "blocking": True, "pass_fail_affected": True}

    diagnostics = synthesis.validate_contract("s06_maintainability_diagnostic", payload, where=where)

    assert {row["diagnostic_code"] for row in diagnostics} == {"riskratchet_blocking_or_not_diagnostic_only"}
    assert diagnostics[0]["failure_source_class"] == "riskratchet"


def test_s05_import_ready_override_is_rejected(tmp_path: Path) -> None:
    where = tmp_path / "decision.json"
    payload = {
        "schema_version": "m027-end-to-end-mixed-replay-readiness-decision.v1",
        "slice_id": "S05",
        "selection_id": synthesis.SELECTION_ID,
        "decision": "import_ready",
        "ready_for_import": True,
    }

    diagnostics = synthesis.validate_contract("s05_readiness_decision", payload, where=where)

    assert {row["diagnostic_code"] for row in diagnostics} == {"readiness_decision_claim_creep"}
    assert diagnostics[0]["failure_source_class"] == "claim_boundary"


def test_raw_payload_key_and_marker_are_rejected(tmp_path: Path) -> None:
    where = tmp_path / "metadata.json"
    payload = {"article_id": "x", "raw_text": "RAW_ARXIV_ABS_SECRET"}

    diagnostics = synthesis.validate_no_payload_leakage(payload, serialized=json.dumps(payload), where=where)

    assert {row["diagnostic_code"] for row in diagnostics} == {"metadata_payload_key_leakage", "metadata_payload_snippet_leakage"}
    assert {row["failure_source_class"] for row in diagnostics} == {"redaction"}


def test_validate_s07_outputs_rejects_stale_output_hash(tmp_path: Path) -> None:
    summary_path, diagnostics_path, report_path, _summary = _valid_s07_outputs(tmp_path)
    report_path.write_text(report_path.read_text(encoding="utf-8") + "tampered\n", encoding="utf-8")

    findings = synthesis.validate_s07_outputs(summary_path, diagnostics_path, report_path, root=tmp_path)

    assert "output_artifact_sha256_mismatch" in {row["diagnostic_code"] for row in findings}
    assert any(row["failure_source_class"] == "provenance_hash" for row in findings)


def test_validate_s07_outputs_rejects_missing_zero_chunk_blocker_in_summary(tmp_path: Path) -> None:
    summary_path, diagnostics_path, report_path, summary = _valid_s07_outputs(tmp_path)
    summary["functional_readiness"]["ready_with_blockers_conditions"] = ["All preprocessing blockers resolved."]
    _write_json(summary_path, summary)

    findings = synthesis.validate_s07_outputs(summary_path, diagnostics_path, report_path, root=tmp_path)

    assert "missing_parser_ready_zero_chunk_blocker" in {row["diagnostic_code"] for row in findings}
    assert any(row["json_path"] == "$.functional_readiness" for row in findings)


def test_validate_s07_outputs_rejects_missing_zero_chunk_blocker_in_report(tmp_path: Path) -> None:
    summary_path, diagnostics_path, report_path, _summary = _valid_s07_outputs(tmp_path)
    report_path.write_text(report_path.read_text(encoding="utf-8").replace("zero chunks", "nonempty chunks"), encoding="utf-8")

    findings = synthesis.validate_s07_outputs(summary_path, diagnostics_path, report_path, root=tmp_path)

    assert "missing_parser_ready_zero_chunk_blocker" in {row["diagnostic_code"] for row in findings}


def test_validate_s07_outputs_rejects_positive_claim_in_summary(tmp_path: Path) -> None:
    summary_path, diagnostics_path, report_path, summary = _valid_s07_outputs(tmp_path)
    summary["functional_readiness"]["ready_now"] = ["The corpus is ready for graph import."]
    _write_json(summary_path, summary)

    findings = synthesis.validate_s07_outputs(summary_path, diagnostics_path, report_path, root=tmp_path)

    assert "forbidden_positive_readiness_claim" in {row["diagnostic_code"] for row in findings}
    assert any(row["failure_source_class"] == "claim_boundary" for row in findings)


def test_validate_s07_outputs_rejects_positive_claim_in_report(tmp_path: Path) -> None:
    summary_path, diagnostics_path, report_path, _summary = _valid_s07_outputs(tmp_path)
    report_path.write_text(report_path.read_text(encoding="utf-8") + "\nThe corpus is production ready.\n", encoding="utf-8")

    findings = synthesis.validate_s07_outputs(summary_path, diagnostics_path, report_path, root=tmp_path)

    assert "forbidden_positive_readiness_claim" in {row["diagnostic_code"] for row in findings}


def test_validate_only_exits_nonzero_for_tampered_s07_outputs(tmp_path: Path, monkeypatch: Any) -> None:
    summary_path, diagnostics_path, report_path, _summary = _valid_s07_outputs(tmp_path)
    report_path.write_text(report_path.read_text(encoding="utf-8") + "\nThe corpus is import ready.\n", encoding="utf-8")
    monkeypatch.setattr(synthesis, "INPUT_ARTIFACTS", ())
    monkeypatch.setattr(synthesis, "SUMMARY_PATH", summary_path)
    monkeypatch.setattr(synthesis, "DIAGNOSTICS_PATH", diagnostics_path)
    monkeypatch.setattr(synthesis, "REPORT_PATH", report_path)

    assert synthesis.main(["--validate-only", "--root", str(tmp_path)]) == 1
