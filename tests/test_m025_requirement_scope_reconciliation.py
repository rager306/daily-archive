from __future__ import annotations

import importlib.util
import json
import sys
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m025_requirement_scope_reconciliation.py"
spec = importlib.util.spec_from_file_location("verify_m025_requirement_scope_reconciliation", MODULE_PATH)
assert spec is not None and spec.loader is not None
verify_m025_requirement_scope_reconciliation = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = verify_m025_requirement_scope_reconciliation
spec.loader.exec_module(verify_m025_requirement_scope_reconciliation)

validate_matrix = verify_m025_requirement_scope_reconciliation.validate_matrix
validate_coverage_handoff = verify_m025_requirement_scope_reconciliation.validate_coverage_handoff
main = verify_m025_requirement_scope_reconciliation.main

REAL_MATRIX = Path(__file__).parents[1] / "doc" / "validation" / "m025_requirement_scope_matrix.json"
REAL_RENDERED = Path(__file__).parents[1] / "doc" / "validation" / "m025_requirement_scope_matrix.md"
REQUIRED_IDS = {"R024", "R027", "R029", "R030", "R036", "R040"}


def _load_real_matrix() -> dict[str, Any]:
    return json.loads(REAL_MATRIX.read_text(encoding="utf-8"))


def _load_rendered() -> str:
    return REAL_RENDERED.read_text(encoding="utf-8")


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def _materialize_non_planning_evidence(tmp_path: Path, matrix: dict[str, Any]) -> None:
    for row in matrix["requirements"]:
        for raw_path in row["evidence_paths"]:
            if raw_path.startswith((".gsd/", ".planning/", ".audits/")):
                continue
            evidence = tmp_path / raw_path
            evidence.parent.mkdir(parents=True, exist_ok=True)
            evidence.write_text("{}\n", encoding="utf-8")


def _errors(matrix: dict[str, Any], rendered: str | None = None, tmp_path: Path | None = None) -> list[str]:
    root = tmp_path or Path(__file__).parents[1]
    if tmp_path is not None:
        _materialize_non_planning_evidence(tmp_path, matrix)
    return validate_matrix(
        matrix,
        rendered if rendered is not None else _load_rendered(),
        repo_root=root,
        required_requirements=REQUIRED_IDS,
        reject_unsafe_claims=True,
        allow_planning_evidence=False,
    )


def _row(matrix: dict[str, Any], requirement_id: str) -> dict[str, Any]:
    return next(row for row in matrix["requirements"] if row["requirement_id"] == requirement_id)


def test_real_matrix_passes_without_reading_project_gsd(tmp_path: Path) -> None:
    matrix = _load_real_matrix()

    errors = _errors(matrix, tmp_path=tmp_path)

    assert errors == []


def test_cli_passes_against_real_matrix_with_manual_closeout_evidence() -> None:
    exit_code = main(
        [
            "--matrix",
            str(REAL_MATRIX),
            "--rendered",
            str(REAL_RENDERED),
            "--require-requirements",
            "R024",
            "R027",
            "R029",
            "R030",
            "R036",
            "R040",
            "--reject-unsafe-claims",
        ]
    )

    assert exit_code == 0


def test_rejects_missing_required_requirement_id(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    matrix["requirements"] = [row for row in matrix["requirements"] if row["requirement_id"] != "R029"]

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("missing requirement rows: R029" in error for error in errors)


def test_rejects_duplicate_requirement_id(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    matrix["requirements"].append(deepcopy(_row(matrix, "R024")))

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("duplicate requirement rows: R024" in error for error in errors)


def test_rejects_malformed_required_row_fields(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    del _row(matrix, "R024")["allowed_claims"]

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("R024 missing required fields: allowed_claims" in error for error in errors)
    assert any("R024 allowed_claims must be a non-empty list of strings" in error for error in errors)


@pytest.mark.parametrize(
    ("bad_path", "expected"),
    [
        ("data/article_corpora/missing-evidence.json", "R024 evidence path does not exist"),
        ("../outside.json", "R024 evidence path must not escape the repo"),
        ("https://example.test/evidence.json", "R024 evidence path must be a repo-relative path"),
    ],
)
def test_rejects_malformed_or_missing_evidence_paths(bad_path: str, expected: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R024")["evidence_paths"] = [bad_path]

    errors = _errors(matrix)

    assert any(expected in error for error in errors)


@pytest.mark.parametrize(
    ("requirement_id", "claim"),
    [
        ("R024", "M025 fully validates R024."),
        ("R027", "M025 validates graph readiness."),
        ("R029", "M025 authorizes KG import readiness."),
        ("R036", "M025 authorizes production LadybugDB writes."),
        ("R040", "M025 fully validates all future infrastructure safety wrapping."),
    ],
)
def test_rejects_unsafe_positive_claim_phrases(tmp_path: Path, requirement_id: str, claim: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, requirement_id)["allowed_claims"].append(claim)

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("unsafe claim phrase" in error and requirement_id in error for error in errors)


@pytest.mark.parametrize(
    ("field", "expected"),
    [
        ("graph_readiness_claim", "unsafe boolean field must not be true"),
        ("import_ready", "unsafe boolean field must not be true"),
        ("production_import_attempted", "unsafe boolean field must not be true"),
        ("raw_article_text_included", "unsafe boolean field must not be true"),
    ],
)
def test_rejects_unsafe_true_boolean_fields(tmp_path: Path, field: str, expected: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R027")[field] = True

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any(expected in error and field in error for error in errors)


@pytest.mark.parametrize("field", ["raw_article_text", "binary_payload", "base64_payload", "vector_payload"])
def test_rejects_raw_payload_field_names(tmp_path: Path, field: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R030")[field] = "payload must not be stored in coverage artifacts"

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("contains unsafe raw/binary/base64/vector/secret field name" in error and field in error for error in errors)


@pytest.mark.parametrize("requirement_id", ["R024", "R027", "R029", "R036"])
def test_rejects_broad_active_requirements_marked_validated(tmp_path: Path, requirement_id: str) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, requirement_id)
    row["current_status"] = "validated"
    row["s11_verdict"] = "validated_by_m025"
    row["allowed_claims"].append(f"M025 fully validates {requirement_id}.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any(requirement_id in error and ("must" in error or "unsafe claim" in error) for error in errors)


def test_rejects_r030_reopened_or_newly_validated(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R030")
    row["current_status"] = "active"
    row["s11_verdict"] = "advanced_not_validated"
    row["allowed_claims"] = ["M025 newly validates R030."]

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("R030 current_status must be validated" in error for error in errors)
    assert any("R030 must be treated as already validated/supported" in error for error in errors)
    assert any("R030 must not be claimed as newly validated" in error for error in errors)


def test_rejects_r040_as_universal_future_safety_validation(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R040")
    row["current_status"] = "validated"
    row["s11_verdict"] = "fully_validated"
    row["allowed_claims"].append("M025 fully validates all future infrastructure safety wrapping.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("R040 current_status must be active" in error for error in errors)
    assert any("R040 must be treated as a followed constraint" in error for error in errors)


def test_rejects_stale_rendered_markdown(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    rendered = _load_rendered().replace("R036", "RXXX")

    errors = _errors(matrix, rendered=rendered, tmp_path=tmp_path)

    assert any("rendered markdown missing requirement id: R036" in error for error in errors)


def test_coverage_handoff_requires_closeout_sections_and_evidence_paths() -> None:
    coverage = "\n".join(
        [
            "R024 R027 R029 R030 R036 R040",
            "doc/validation/m025_requirement_scope_matrix.json",
            "doc/validation/m025_requirement_scope_matrix.md",
            "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json",
            "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-report.md",
            "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json",
            "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-summary.json",
            "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-report.md",
            "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-events.jsonl",
            "m025_advanced_preprocessing_only",
            "m025_advanced_preprocessing_diagnostics",
            "m025_advanced_traceable_chunks",
            "already_validated_supported_by_m025",
            "m025_advanced_audit_provenance",
            "constraint_followed_not_validated",
            "Q5 — Failure Modes",
            "Q6 — Load Profile",
            "Q7 — Negative Tests",
        ]
    )

    assert validate_coverage_handoff(coverage) == []


def test_coverage_handoff_rejects_missing_required_evidence_path() -> None:
    errors = validate_coverage_handoff("R024 R027 R029 R030 R036 R040")

    assert any("coverage handoff missing required phrase/path" in error for error in errors)
