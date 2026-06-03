from __future__ import annotations

import importlib.util
import json
import sys
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

MODULE_PATH = Path(__file__).parents[1] / "scripts" / "verify_m028_requirement_scope_reconciliation.py"
spec = importlib.util.spec_from_file_location("verify_m028_requirement_scope_reconciliation", MODULE_PATH)
assert spec is not None and spec.loader is not None
verify_m028_requirement_scope_reconciliation = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = verify_m028_requirement_scope_reconciliation
spec.loader.exec_module(verify_m028_requirement_scope_reconciliation)

validate_matrix = verify_m028_requirement_scope_reconciliation.validate_matrix
main = verify_m028_requirement_scope_reconciliation.main
REQUIRED_IDS = verify_m028_requirement_scope_reconciliation.REQUIRED_REQUIREMENT_IDS

REAL_MATRIX = Path(__file__).parents[1] / "doc" / "validation" / "m028_requirement_scope_matrix.json"
REAL_RENDERED = Path(__file__).parents[1] / "doc" / "validation" / "m028_requirement_scope_matrix.md"


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


@pytest.mark.parametrize("requirement_id", ["R027", "R035"])
def test_rejects_missing_required_requirement_id(tmp_path: Path, requirement_id: str) -> None:
    matrix = _load_real_matrix()
    matrix["requirements"] = [row for row in matrix["requirements"] if row["requirement_id"] != requirement_id]

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any(
        "M028_MATRIX_REQUIRED_ROW_MISSING" in error and f"missing requirement rows: {requirement_id}" in error
        for error in errors
    )


def test_rejects_duplicate_requirement_id(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    matrix["requirements"].append(deepcopy(_row(matrix, "R024")))

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("M028_MATRIX_REQUIRED_ROW_DUPLICATE" in error and "duplicate requirement rows: R024" in error for error in errors)


@pytest.mark.parametrize(
    ("bad_path", "expected"),
    [
        ("data/article_corpora/missing-evidence.json", "evidence path does not exist"),
        ("../outside.json", "evidence path must not escape the repo"),
        ("https://example.test/evidence.json", "evidence path must be a repo-relative path"),
        ("data/article_corpora/evidence.bin", "evidence path has unsupported extension"),
    ],
)
def test_rejects_malformed_or_missing_evidence_paths(bad_path: str, expected: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R024")["evidence_paths"] = [bad_path]

    errors = _errors(matrix)

    assert any("M028_MATRIX_EVIDENCE_PATH_INVALID" in error and expected in error for error in errors)


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

    assert any("M028_MATRIX_EVIDENCE_PATH_INVALID" in error and ".gsd/does/not/exist.md" in error for error in errors)


@pytest.mark.parametrize(
    ("requirement_id", "claim"),
    [
        ("R024", "M028 globally validates R024."),
        ("R027", "M028 validates graph readiness."),
        ("R029", "M028 validates import-ready chunks."),
        ("R019", "M028 fully validates R019."),
        ("R023", "M028 promotes trusted facts."),
        ("R031", "M028 proves unattended scaling."),
        ("R033", "M028 validates Scientific KG corpus behavior."),
        ("R035", "M028 fully validates R035."),
        ("R035", "M028 advances R035 as a deliverable."),
        ("R035", "M028 delivers validation-batch quota top-up."),
        ("R035", "M028 materializes deterministic replacement candidates."),
        ("R051", "M028 activates MiniMax."),
    ],
)
def test_rejects_unsafe_positive_claim_phrases(tmp_path: Path, requirement_id: str, claim: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, requirement_id)["allowed_claims"].append(claim)

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("M028_MATRIX_UNSAFE_CLAIM_LEAKED" in error and "unsafe claim phrase" in error and requirement_id in error for error in errors)


@pytest.mark.parametrize(
    "field",
    [
        "kg_import_or_readiness_claimed",
        "graph_validation_claimed",
        "trusted_fact_promotion_claimed",
        "production_ladybugdb_writes_claimed",
        "parser_or_chunker_changed",
        "raw_payloads_embedded",
    ],
)
def test_rejects_unsafe_true_boolean_fields(tmp_path: Path, field: str) -> None:
    matrix = _load_real_matrix()
    matrix["safety_flags"][field] = True

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("M028_MATRIX_UNSAFE_FLAG_TRUE" in error and field in error for error in errors)


@pytest.mark.parametrize(
    "field",
    ["raw_article_text", "binary_payload", "base64_payload", "vector_payload", "secret_value", "production_connection"],
)
def test_rejects_raw_payload_or_secret_field_names(tmp_path: Path, field: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R036")[field] = "must not be stored in validation metadata"

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("M028_MATRIX_UNSAFE_CLAIM_LEAKED" in error and "field name" in error and field in error for error in errors)


@pytest.mark.parametrize(
    "marker",
    ["-----BEGIN PRIVATE KEY-----", "data:application/pdf;base64,AAAA", "secret=do-not-store", "password=do-not-store"],
)
def test_rejects_raw_payload_base64_or_secret_leakage_markers(tmp_path: Path, marker: str) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R024")["review_notes"] = marker

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("M028_MATRIX_UNSAFE_CLAIM_LEAKED" in error and "secret leakage marker" in error for error in errors)


@pytest.mark.parametrize("requirement_id", ["R019", "R022", "R023", "R031", "R032", "R033", "R035", "R050", "R051", "R052"])
def test_rejects_false_validation_of_future_out_of_scope_requirements(tmp_path: Path, requirement_id: str) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, requirement_id)
    row["current_status"] = "validated"
    row["s07_verdict"] = "validated_by_m028"
    row["allowed_claims"].append(f"M028 fully validates {requirement_id}.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("M028_MATRIX_UNSAFE_CLAIM_LEAKED" in error and requirement_id in error for error in errors)


def test_rejects_false_global_validation_of_smoke_loader_requirement(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R024")
    row["s07_verdict"] = "globally_validated"
    row["allowed_claims"].append("M028 globally validates R024.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("$.requirements" in error and "R024 s07_verdict" in error for error in errors)
    assert any("unsafe claim phrase" in error and "R024" in error for error in errors)


def test_rejects_r036_false_global_validation_claim(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R036")
    row["s07_verdict"] = "globally_validated"
    row["allowed_claims"].append("M028 newly validates R036 globally.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("M028_MATRIX_UNSAFE_CLAIM_LEAKED" in error and "R036" in error for error in errors)
    assert any("unsafe claim phrase" in error and "R036" in error for error in errors)


def test_rejects_r040_false_global_validation_claim(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R040")
    row["current_status"] = "validated"
    row["allowed_claims"].append("M028 fully validates R040.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("M028_MATRIX_UNSAFE_CLAIM_LEAKED" in error and "R040" in error for error in errors)
    assert any("unsafe claim phrase" in error and "R040" in error for error in errors)


def test_rejects_stale_rendered_markdown(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    rendered = _load_rendered().replace("R036", "RXXX")

    errors = _errors(matrix, rendered=rendered, tmp_path=tmp_path)

    assert any("M028_MATRIX_MARKDOWN_STALE" in error and "rendered markdown missing requirement id: R036" in error for error in errors)


def test_rejects_rendered_markdown_with_stale_source_matrix_path(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    rendered = _load_rendered().replace("doc/validation/m028_requirement_scope_matrix.json", "doc/validation/old.json")

    errors = _errors(matrix, rendered=rendered, tmp_path=tmp_path)

    assert any("M028_MATRIX_MARKDOWN_STALE" in error and "doc/validation/m028_requirement_scope_matrix.json" in error for error in errors)


def test_cli_rejects_negative_fixture(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R027")["allowed_claims"].append("M028 validates graph readiness.")
    matrix_path = tmp_path / "matrix.json"
    rendered_path = tmp_path / "rendered.md"
    _write_json(matrix_path, matrix)
    rendered_path.write_text(_load_rendered(), encoding="utf-8")

    exit_code = main(["--matrix", str(matrix_path), "--rendered", str(rendered_path), "--validate-only"])

    assert exit_code == 1
