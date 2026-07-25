from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any

from scripts.verify_m031_validation_remediation import (
    OUTPUT_DIR,
    REQUIRED_FALSE_FLAGS,
    M031ValidationRemediationError,
    _json_path,
    _repo_relative_path,
    _safe_output_path,
    build_evidence,
    build_runtime_diagnostics,
    main,
    validate_diagnostics_rows,
    validate_evidence,
)


def _assessment() -> str:
    return """---
sliceId: S02
uatType: artifact-driven
verdict: FAIL
---

# UAT Result — S02

The required regression pytest suite check did not produce the expected passing result.
A full `uv run pytest -q` run failed during collection with unrelated bounded chunk repair ImportError errors.
"""


def _summary() -> str:
    return """# S02 Summary

Fresh closeout verification produced `65 passed in 1.25s` for the scoped regression suite.
No graph import, production import, or LadybugDB writes were attempted.
"""


def _uat() -> str:
    return """# S02 UAT

Expected Outcomes include: The regression suite passes with `65 passed` and ruff reports `All checks passed!`.
"""


def _replay_closeout() -> dict[str, Any]:
    return {
        "status": "passed",
        "counts": {"failed": 0},
        "fail_closed_safety_flags": dict.fromkeys(REQUIRED_FALSE_FLAGS, False),
    }


def _s05_closeout() -> dict[str, Any]:
    return {
        "status": "passed",
        "accepted_count": 0,
        "import_eligible_count": 0,
        "fail_closed_flags": dict.fromkeys(REQUIRED_FALSE_FLAGS, False),
    }


def _matrix() -> dict[str, Any]:
    return {
        "row_count": 7,
        "fail_closed_flags": dict.fromkeys(REQUIRED_FALSE_FLAGS, False),
    }


def _audit() -> dict[str, Any]:
    return {
        "schema_version": "m031-process-continuity-audit.v1",
        "review_verdict_state": {"completed_review_event_count": 0, "verdict_event_count": 0},
        "fail_closed_flags": dict.fromkeys(REQUIRED_FALSE_FLAGS, False),
    }


def _review_events() -> list[dict[str, Any]]:
    flags = dict.fromkeys(REQUIRED_FALSE_FLAGS, False)
    return [
        {
            "event": "independent_review.requested",
            "independent_review_completed": False,
            "output_contract_completed": False,
            "fail_closed_safety_flags": flags,
        },
        {
            "event": "independent_review.summary",
            "independent_review_completed": False,
            "output_contract_completed": False,
            "fail_closed_safety_flags": flags,
        },
    ]


def _evidence() -> dict[str, Any]:
    return build_evidence(
        s02_assessment=_assessment(),
        s02_summary=_summary(),
        s02_uat=_uat(),
        replay_closeout=_replay_closeout(),
        s05_closeout=_s05_closeout(),
        progression_matrix=_matrix(),
        continuity_audit=_audit(),
        review_events=_review_events(),
    )


def test_m186_json_and_repo_path_helpers_are_fail_closed(tmp_path: Path) -> None:
    existing = tmp_path / "inputs" / "evidence.json"
    existing.parent.mkdir(parents=True)
    existing.write_text("{}", encoding="utf-8")

    assert _json_path("$.rows", 0) == "$.rows[0]"
    assert _json_path("$", "safety_flags") == "$.safety_flags"
    assert _repo_relative_path(
        "inputs/evidence.json", repo_root=tmp_path, label="input"
    ) == existing.resolve()
    assert _safe_output_path(
        OUTPUT_DIR / "evidence.json", repo_root=tmp_path, label="output"
    ) == (tmp_path / OUTPUT_DIR / "evidence.json").resolve()

    for unsafe in ("", " inputs/evidence.json", "../evidence.json", "/tmp/evidence.json", "https://example.test/evidence.json"):
        try:
            _repo_relative_path(unsafe, repo_root=tmp_path, label="input")
        except M031ValidationRemediationError:
            pass
        else:  # pragma: no cover - assertion message is the contract
            raise AssertionError(f"unsafe repo-relative path accepted: {unsafe!r}")

    try:
        _safe_output_path("tmp/evidence.json", repo_root=tmp_path, label="output")
    except M031ValidationRemediationError:
        pass
    else:  # pragma: no cover - assertion message is the contract
        raise AssertionError("output outside validation remediation directory accepted")


def test_m186_build_evidence_remains_metadata_only_and_fail_closed() -> None:
    evidence = _evidence()

    assert evidence["metadata_only"] is True
    assert evidence["graph_import_boundary"]["completed_review_refusal_in_force"] is True
    for key in REQUIRED_FALSE_FLAGS:
        assert evidence["safety_flags"][key] is False
    for key in (
        "requirement_records_modified",
        "graph_or_import_writes_enabled",
        "source_write_attempted",
        "non_artifact_write_attempted",
        "import_ready_claimed",
        "trusted_fact_promotion_allowed",
        "model_call_attempted",
        "secrets_included",
    ):
        assert evidence["safety_flags"][key] is False


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")


def _write_fixture_tree(
    root: Path,
    *,
    summary: str | None = None,
    uat: str | None = None,
    malformed_json: bool = False,
    malformed_jsonl: bool = False,
) -> dict[str, Path]:
    paths = {
        "assessment": root / "inputs" / "s02-assessment.md",
        "summary": root / "inputs" / "s02-summary.md",
        "uat": root / "inputs" / "s02-uat.md",
        "replay": root / "inputs" / "replay-closeout-summary.json",
        "s05": root / "inputs" / "s05-closeout-summary.json",
        "matrix": root / "inputs" / "progression-matrix.json",
        "audit": root / "inputs" / "m031-continuity-audit.json",
        "events": root / "inputs" / "independent-review-events.jsonl",
    }
    paths["assessment"].parent.mkdir(parents=True, exist_ok=True)
    paths["assessment"].write_text(_assessment(), encoding="utf-8")
    paths["summary"].write_text(_summary() if summary is None else summary, encoding="utf-8")
    paths["uat"].write_text(_uat() if uat is None else uat, encoding="utf-8")
    if malformed_json:
        paths["replay"].write_text("{not-json", encoding="utf-8")
    else:
        _write_json(paths["replay"], _replay_closeout())
    _write_json(paths["s05"], _s05_closeout())
    _write_json(paths["matrix"], _matrix())
    _write_json(paths["audit"], _audit())
    if malformed_jsonl:
        paths["events"].parent.mkdir(parents=True, exist_ok=True)
        paths["events"].write_text("{not-json\n", encoding="utf-8")
    else:
        _write_jsonl(paths["events"], _review_events())
    return paths


def _args(paths: dict[str, Path], *extra: str) -> list[str]:
    return [
        "--s02-assessment",
        paths["assessment"].as_posix(),
        "--s02-summary",
        paths["summary"].as_posix(),
        "--s02-uat",
        paths["uat"].as_posix(),
        "--replay-closeout",
        paths["replay"].as_posix(),
        "--s05-closeout",
        paths["s05"].as_posix(),
        "--progression-matrix",
        paths["matrix"].as_posix(),
        "--continuity-audit",
        paths["audit"].as_posix(),
        "--review-events",
        paths["events"].as_posix(),
        *extra,
    ]


def _relative_paths(root: Path, paths: dict[str, Path]) -> dict[str, Path]:
    return {key: path.relative_to(root) for key, path in paths.items()}


def test_positive_cli_writes_requested_outputs_under_validation_remediation(
    tmp_path: Path, monkeypatch
) -> None:
    paths = _relative_paths(tmp_path, _write_fixture_tree(tmp_path))
    monkeypatch.chdir(tmp_path)
    evidence_out = OUTPUT_DIR / "evidence.json"
    diagnostics_out = OUTPUT_DIR / "diagnostics.jsonl"
    report_out = OUTPUT_DIR / "report.md"
    summary_out = OUTPUT_DIR / "verify-summary.json"

    exit_code = main(
        _args(
            paths,
            "--write-evidence",
            evidence_out.as_posix(),
            "--write-diagnostics",
            diagnostics_out.as_posix(),
            "--write-report",
            report_out.as_posix(),
            "--write-verify-summary",
            summary_out.as_posix(),
            "--validate-only",
        )
    )

    assert exit_code == 0
    assert (
        json.loads(evidence_out.read_text(encoding="utf-8"))["s02_assessment_reconciliation"][
            "fresh_65_pass_evidence_present"
        ]
        is True
    )
    report = report_out.read_text(encoding="utf-8")
    assert "M031 Validation Remediation Dossier" in report
    assert "## Reader Action" in report
    assert "Fresh `65 passed` evidence present: `True`" in report
    assert "## Forbidden Claims" in report
    assert "S06 does not enable production graph import or LadybugDB writes" in report
    assert "## Milestone Validation Handoff Snippets" in report
    assert json.loads(summary_out.read_text(encoding="utf-8"))["status"] == "passed"
    diagnostic_rows = [
        json.loads(line) for line in diagnostics_out.read_text(encoding="utf-8").splitlines()
    ]
    assert [row["code"] for row in diagnostic_rows].count(
        "M031_VALIDATION_REMEDIATION_STALE_S02_ASSESSMENT_RECONCILED"
    ) == 1


def test_rejects_stale_s02_failure_without_fresh_65_pass_evidence() -> None:
    evidence = _evidence()
    evidence["s02_assessment_reconciliation"]["fresh_65_pass_evidence_present"] = False
    evidence["s02_assessment_reconciliation"]["fresh_65_pass_evidence_sources"] = []

    errors = validate_evidence(evidence)

    assert any(
        error["code"] == "M031_VALIDATION_REMEDIATION_MISSING_S02_65_PASS_EVIDENCE"
        for error in errors
    )


def test_cli_rejects_missing_65_pass_evidence_before_writes(tmp_path: Path, monkeypatch) -> None:
    paths = _relative_paths(
        tmp_path,
        _write_fixture_tree(
            tmp_path, summary="# S02 Summary\n36 passed only\n", uat="# S02 UAT\n36 passed only\n"
        ),
    )
    monkeypatch.chdir(tmp_path)
    output = OUTPUT_DIR / "evidence.json"

    exit_code = main(_args(paths, "--write-evidence", output.as_posix()))

    assert exit_code == 1
    assert not output.exists()


def test_rejects_missing_requirement_rows() -> None:
    evidence = _evidence()
    evidence["requirement_coverage"] = [
        row for row in evidence["requirement_coverage"] if row["requirement_id"] != "R050"
    ]

    errors = validate_evidence(evidence)

    assert any(
        error["code"] == "M031_VALIDATION_REMEDIATION_MISSING_REQUIREMENT_ROW" for error in errors
    )


def test_rejects_missing_canonical_class_rows() -> None:
    evidence = _evidence()
    evidence["canonical_verification_classes"] = [
        row for row in evidence["canonical_verification_classes"] if row["class"] != "UAT"
    ]

    errors = validate_evidence(evidence)

    assert any(error["code"] == "M031_VALIDATION_REMEDIATION_MISSING_CLASS_ROW" for error in errors)


def test_rejects_unsafe_true_flags() -> None:
    evidence = _evidence()
    evidence["safety_flags"]["ladybugdb_written"] = True

    errors = validate_evidence(evidence)

    assert any(error["code"] == "M031_VALIDATION_REMEDIATION_UNSAFE_FLAG_TRUE" for error in errors)


def test_rejects_raw_payload_and_key_leakage() -> None:
    evidence = _evidence()
    evidence["raw_article_text"] = "<html>raw article payload</html>"
    evidence["api_key"] = "api_key=secret"

    errors = validate_evidence(evidence)

    assert any(
        error["code"] == "M031_VALIDATION_REMEDIATION_RAW_PAYLOAD_LEAKAGE" for error in errors
    )


def test_rejects_permissive_graph_import_claims() -> None:
    evidence = _evidence()
    evidence["graph_import_boundary"]["completed_review_refusal_in_force"] = False
    evidence["graph_import_boundary"]["accepted_count"] = 1
    evidence["graph_import_boundary"]["safe_claim"] = "M031 is ready for graph import"

    errors = validate_evidence(evidence)

    assert any(
        error["code"] == "M031_VALIDATION_REMEDIATION_PERMISSIVE_GRAPH_IMPORT_CLAIM"
        for error in errors
    )
    assert any(
        error["code"] == "M031_VALIDATION_REMEDIATION_FORBIDDEN_POSITIVE_CLAIM" for error in errors
    )


def test_rejects_malformed_diagnostics() -> None:
    diagnostics = build_runtime_diagnostics(_evidence())
    malformed = deepcopy(diagnostics)
    malformed[0].pop("code")
    malformed[0]["network_fetch_attempted"] = True

    errors = validate_diagnostics_rows(malformed)

    assert any(
        error["code"] == "M031_VALIDATION_REMEDIATION_MALFORMED_DIAGNOSTIC" for error in errors
    )


def test_cli_rejects_malformed_json_and_jsonl(tmp_path: Path, monkeypatch) -> None:
    malformed_json_paths = _relative_paths(
        tmp_path, _write_fixture_tree(tmp_path, malformed_json=True)
    )
    monkeypatch.chdir(tmp_path)
    assert main(_args(malformed_json_paths)) == 2

    malformed_jsonl_paths = _relative_paths(
        tmp_path, _write_fixture_tree(tmp_path, malformed_jsonl=True)
    )
    assert main(_args(malformed_jsonl_paths)) == 2


def test_rejects_path_traversal_and_out_of_corpus_outputs(tmp_path: Path, monkeypatch) -> None:
    paths = _relative_paths(tmp_path, _write_fixture_tree(tmp_path))
    monkeypatch.chdir(tmp_path)

    assert main(_args(paths, "--write-evidence", "../outside.json")) == 2
    assert (
        main(
            _args(
                paths,
                "--write-evidence",
                "data/article_corpora/m031-catalog-backed-replay-v1/not-remediation/evidence.json",
            )
        )
        == 2
    )


def test_validate_only_without_write_creates_no_output_directory(
    tmp_path: Path, monkeypatch
) -> None:
    paths = _relative_paths(tmp_path, _write_fixture_tree(tmp_path))
    monkeypatch.chdir(tmp_path)

    assert main(_args(paths, "--validate-only")) == 0
    assert not OUTPUT_DIR.exists()
