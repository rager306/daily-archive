from __future__ import annotations

import copy
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_m024_validation_evidence_closure.py"

spec = importlib.util.spec_from_file_location("verify_m024_validation_evidence_closure", SCRIPT)
assert spec and spec.loader
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)


def _load_real() -> tuple[dict[str, Any], dict[str, Any]]:
    matrix = json.loads((ROOT / "doc/validation/m024_requirement_coverage_matrix.json").read_text())
    closure = json.loads((ROOT / "doc/validation/m024_validation_evidence_closure.json").read_text())
    return matrix, closure


def _write_inputs(tmp_path: Path, matrix: dict[str, Any], closure: dict[str, Any]) -> tuple[Path, Path]:
    matrix_path = tmp_path / "matrix.json"
    closure_path = tmp_path / "closure.json"
    closure.setdefault("source_matrix", {})["json_path"] = str(matrix_path)
    matrix_path.write_text(json.dumps(matrix))
    closure_path.write_text(json.dumps(closure))
    return matrix_path, closure_path


def _run(tmp_path: Path, matrix: dict[str, Any], closure: dict[str, Any]) -> subprocess.CompletedProcess[str]:
    matrix_path, closure_path = _write_inputs(tmp_path, matrix, closure)
    return subprocess.run(
        [sys.executable, str(SCRIPT), str(matrix_path), str(closure_path)],
        text=True,
        capture_output=True,
        check=False,
    )


def _rows_by_id(rows: list[dict[str, Any]], key: str) -> dict[str, dict[str, Any]]:
    return {row[key]: row for row in rows}


def test_real_m024_validation_evidence_closure_passes() -> None:
    matrix, closure = _load_real()

    errors = verifier.validate_closure(
        matrix,
        closure,
        Path("doc/validation/m024_requirement_coverage_matrix.json"),
    )

    assert errors == []


def test_cli_accepts_valid_temp_fixture(tmp_path: Path) -> None:
    matrix, closure = _load_real()

    result = _run(tmp_path, matrix, closure)

    assert result.returncode == 0, result.stderr
    assert "validation passed" in result.stdout


def test_rejects_missing_riskratchet_closure(tmp_path: Path) -> None:
    matrix, closure = _load_real()
    closure["closure_decisions"] = [
        row for row in closure["closure_decisions"] if row["gap_id"] != "S09-GAP-riskratchet-direct-evidence"
    ]

    result = _run(tmp_path, matrix, closure)

    assert result.returncode == 1
    assert "missing decision for S09-GAP-riskratchet-direct-evidence" in result.stderr


@pytest.mark.parametrize(
    ("field", "value", "expected"),
    [
        ("blocking_for_m024_validation_rerun", True, "must not block"),
        ("riskratchet_installed_or_required", True, "must not require or install"),
    ],
)
def test_rejects_riskratchet_blocking_or_required_for_local_progress(
    tmp_path: Path,
    field: str,
    value: bool,
    expected: str,
) -> None:
    matrix, closure = _load_real()
    decisions = _rows_by_id(closure["closure_decisions"], "gap_id")
    decisions["S09-GAP-riskratchet-direct-evidence"][field] = value

    result = _run(tmp_path, matrix, closure)

    assert result.returncode == 1
    assert expected in result.stderr


@pytest.mark.parametrize("requirement_id", ["R024", "R027", "R029"])
def test_rejects_partial_requirements_changed_to_validated(tmp_path: Path, requirement_id: str) -> None:
    matrix, closure = _load_real()
    matrix_rows = _rows_by_id(matrix["requirements"], "requirement_id")
    closure_rows = _rows_by_id(closure["requirement_treatments"], "requirement_id")
    matrix_rows[requirement_id]["coverage_verdict"] = "covered_by_existing_validation"
    closure_rows[requirement_id]["m024_treatment"] = "validated"
    closure_rows[requirement_id]["validation_rerun_position"] = "fully_validated"

    result = _run(tmp_path, matrix, closure)

    assert result.returncode == 1
    assert f"matrix {requirement_id} must remain advanced_not_validated" in result.stderr
    assert f"closure {requirement_id} must use advanced_not_validated treatment" in result.stderr


def test_rejects_r030_without_s04_coverage_citation(tmp_path: Path) -> None:
    matrix, closure = _load_real()
    r030 = _rows_by_id(closure["requirement_treatments"], "requirement_id")["R030"]
    r030["evidence_paths"] = ["doc/validation/m024_requirement_coverage_matrix.json"]
    r030["rationale"] = "R030 is covered by existing metadata-only asset evidence."

    result = _run(tmp_path, matrix, closure)

    assert result.returncode == 1
    assert "R030 must cite S04 coverage evidence" in result.stderr


def test_rejects_r036_manual_status_parity_claim(tmp_path: Path) -> None:
    matrix, closure = _load_real()
    r036 = _rows_by_id(closure["requirement_treatments"], "requirement_id")["R036"]
    r036["allowed_claims"] = ["R036 canonical status parity is complete without DB-backed requirement tooling."]

    result = _run(tmp_path, matrix, closure)

    assert result.returncode == 1
    assert "unsafe positive validation claim" in result.stderr
    assert "canonical status parity is complete without db-backed requirement tooling" in result.stderr


def test_rejects_malformed_json(tmp_path: Path) -> None:
    matrix, closure = _load_real()
    matrix_path, closure_path = _write_inputs(tmp_path, matrix, closure)
    closure_path.write_text("{not json")

    result = subprocess.run(
        [sys.executable, str(SCRIPT), str(matrix_path), str(closure_path)],
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 2
    assert "malformed closure JSON" in result.stderr


def test_rejects_missing_matrix_handoff_gap(tmp_path: Path) -> None:
    matrix, closure = _load_real()
    matrix["s09_handoff_gaps"] = [
        row for row in matrix["s09_handoff_gaps"] if row["gap_id"] != "S09-GAP-riskratchet-direct-evidence"
    ]

    result = _run(tmp_path, matrix, closure)

    assert result.returncode == 1
    assert "matrix missing handoff gap S09-GAP-riskratchet-direct-evidence" in result.stderr


@pytest.mark.parametrize(
    "unsafe_claim",
    [
        "M024 authorizes KG import.",
        "M024 validates positive graph readiness.",
    ],
)
def test_rejects_unsafe_approval_phrases_in_positive_claims(tmp_path: Path, unsafe_claim: str) -> None:
    matrix, closure = _load_real()
    mutated = copy.deepcopy(closure)
    mutated["global_allowed_claims"].append(unsafe_claim)

    result = _run(tmp_path, matrix, mutated)

    assert result.returncode == 1
    assert "global_allowed_claims contains unsafe positive validation claim" in result.stderr
