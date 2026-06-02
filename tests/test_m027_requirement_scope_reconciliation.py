from __future__ import annotations

import importlib.util
import json
import sys
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m027_requirement_scope_reconciliation.py"
spec = importlib.util.spec_from_file_location("verify_m027_requirement_scope_reconciliation", MODULE_PATH)
assert spec is not None and spec.loader is not None
verify_m027_requirement_scope_reconciliation = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = verify_m027_requirement_scope_reconciliation
spec.loader.exec_module(verify_m027_requirement_scope_reconciliation)

validate_matrix = verify_m027_requirement_scope_reconciliation.validate_matrix
main = verify_m027_requirement_scope_reconciliation.main
REQUIRED_IDS = verify_m027_requirement_scope_reconciliation.REQUIRED_REQUIREMENT_IDS

REAL_MATRIX = Path(__file__).parents[1] / "doc" / "validation" / "m027_requirement_scope_matrix.json"
REAL_RENDERED = Path(__file__).parents[1] / "doc" / "validation" / "m027_requirement_scope_matrix.md"


def _load_real_matrix() -> dict[str, Any]:
    return json.loads(REAL_MATRIX.read_text(encoding="utf-8"))


def _load_rendered() -> str:
    return REAL_RENDERED.read_text(encoding="utf-8")


def _row(matrix: dict[str, Any], requirement_id: str) -> dict[str, Any]:
    return next(row for row in matrix["requirements"] if row["requirement_id"] == requirement_id)


def _materialize_non_planning_evidence(tmp_path: Path, matrix: dict[str, Any]) -> None:
    paths: set[str] = set()
    paths.update(matrix.get("source_input_paths", []))
    for row in matrix["requirements"]:
        paths.update(row["evidence_paths"])
    for raw_path in paths:
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
        require_planning_evidence=False,
    )


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def test_real_matrix_passes_without_reading_project_gsd(tmp_path: Path) -> None:
    matrix = _load_real_matrix()

    errors = _errors(matrix, tmp_path=tmp_path)

    assert errors == []


def test_cli_passes_default_validate_only() -> None:
    exit_code = main(["--validate-only"])

    assert exit_code == 0


def test_cli_rejects_malformed_json(tmp_path: Path) -> None:
    malformed = tmp_path / "matrix.json"
    malformed.write_text("{not json", encoding="utf-8")

    exit_code = main(["--matrix", str(malformed), "--rendered", str(REAL_RENDERED), "--validate-only"])

    assert exit_code == 2


def test_rejects_missing_required_requirement_id(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    matrix["requirements"] = [row for row in matrix["requirements"] if row["requirement_id"] != "R027"]

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("missing requirement rows: R027" in error for error in errors)


def test_rejects_duplicate_requirement_id(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    matrix["requirements"].append(deepcopy(_row(matrix, "R024")))

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("duplicate requirement rows: R024" in error for error in errors)


@pytest.mark.parametrize(
    ("bad_path", "expected"),
    [
        ("data/article_corpora/missing-evidence.json", "R024 evidence path does not exist"),
        ("../outside.json", "R024 evidence path must not escape the repo"),
        ("https://example.test/evidence.json", "R024 evidence path must be a repo-relative path"),
        ("data/article_corpora/evidence.bin", "R024 evidence path has unsupported extension"),
    ],
)
def test_rejects_malformed_or_missing_evidence_paths(bad_path: str, expected: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R024")["evidence_paths"] = [bad_path]

    errors = _errors(matrix)

    assert any(expected in error for error in errors)


def test_skips_planning_evidence_existence_by_default(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R024")["evidence_paths"] = [".gsd/does/not/exist.md"]

    errors = _errors(matrix, tmp_path=tmp_path)

    assert not any("does not exist" in error for error in errors)


def test_can_require_planning_evidence_existence(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R024")["evidence_paths"] = [".gsd/does/not/exist.md"]

    errors = validate_matrix(
        matrix,
        _load_rendered(),
        repo_root=tmp_path,
        required_requirements=REQUIRED_IDS,
        reject_unsafe_claims=True,
        require_planning_evidence=True,
    )

    assert any("R024 evidence path does not exist: .gsd/does/not/exist.md" in error for error in errors)


@pytest.mark.parametrize(
    ("requirement_id", "claim"),
    [
        ("R024", "M027 globally validates R024."),
        ("R027", "M027 validates graph readiness."),
        ("R029", "M027 validates import-ready chunks."),
        ("R019", "M027 fully validates R019."),
        ("R031", "M027 proves unattended scaling."),
        ("R033", "M027 validates Scientific KG corpus behavior."),
    ],
)
def test_rejects_unsafe_positive_claim_phrases(tmp_path: Path, requirement_id: str, claim: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, requirement_id)["allowed_claims"].append(claim)

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("unsafe claim phrase" in error and requirement_id in error for error in errors)


@pytest.mark.parametrize(
    "field",
    [
        "kg_import_or_readiness_claimed",
        "graph_validation_claimed",
        "trusted_fact_promotion_claimed",
        "production_ladybugdb_writes_claimed",
        "raw_payloads_embedded",
    ],
)
def test_rejects_unsafe_true_boolean_fields(tmp_path: Path, field: str) -> None:
    matrix = _load_real_matrix()
    matrix["safety_flags"][field] = True

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("unsafe boolean field must not be true" in error and field in error for error in errors) or any(
        f"$.safety_flags.{field} must be false" in error for error in errors
    )


@pytest.mark.parametrize("field", ["raw_article_text", "base64_payload", "vector_payload", "secret_value", "production_connection"])
def test_rejects_raw_payload_or_secret_field_names(tmp_path: Path, field: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R036")[field] = "must not be stored in validation metadata"

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("contains unsafe raw/binary/base64/vector/secret field name" in error and field in error for error in errors)


@pytest.mark.parametrize("requirement_id", ["R019", "R022", "R023", "R031", "R032", "R033"])
def test_rejects_false_validation_of_future_out_of_scope_requirements(tmp_path: Path, requirement_id: str) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, requirement_id)
    row["current_status"] = "validated"
    row["s08_verdict"] = "validated_by_m027"
    row["allowed_claims"].append(f"M027 fully validates {requirement_id}.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any(requirement_id in error and ("must remain active" in error or "unsafe claim" in error) for error in errors)


def test_rejects_false_global_validation_of_advanced_requirement(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R024")
    row["s08_verdict"] = "globally_validated"
    row["allowed_claims"].append("M027 globally validates R024.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("R024 s08_verdict must be advanced_not_globally_validated" in error for error in errors)
    assert any("unsafe claim phrase" in error and "R024" in error for error in errors)


def test_rejects_r036_chain_loss(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R036")
    row["allowed_claims"] = ["M027 preserves generic provenance evidence."]
    row["observed_m027_evidence"] = ["Command and hashes are present."]
    row["rationale"] = "generic generated-artifact provenance is present"

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("R036 must preserve the S07 closeout validation chain" in error for error in errors)


def test_rejects_stale_rendered_markdown(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    rendered = _load_rendered().replace("R036", "RXXX")

    errors = _errors(matrix, rendered=rendered, tmp_path=tmp_path)

    assert any("rendered markdown missing requirement id: R036" in error for error in errors)


def test_cli_rejects_negative_fixture(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R027")["allowed_claims"].append("M027 validates graph readiness.")
    matrix_path = tmp_path / "matrix.json"
    rendered_path = tmp_path / "rendered.md"
    _write_json(matrix_path, matrix)
    rendered_path.write_text(_load_rendered(), encoding="utf-8")

    exit_code = main(["--matrix", str(matrix_path), "--rendered", str(rendered_path), "--validate-only"])

    assert exit_code == 1
