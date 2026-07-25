from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

from scripts.verify_m026_requirement_scope_reconciliation import (
    REQUIRED_REQUIREMENT_IDS as REQUIRED_IDS,
)
from scripts.verify_m026_requirement_scope_reconciliation import (
    main,
    validate_coverage_markdown,
    validate_matrix,
)

REAL_MATRIX = (
    Path(__file__).parents[1] / "doc" / "validation" / "m026_requirement_scope_matrix.json"
)
REAL_RENDERED = (
    Path(__file__).parents[1] / "doc" / "validation" / "m026_requirement_scope_matrix.md"
)
REAL_COVERAGE = (
    Path(__file__).parents[1]
    / ".gsd"
    / "milestones"
    / "M026-3rvvgp"
    / "slices"
    / "S05"
    / "S05-COVERAGE.md"
)


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


def _errors(
    matrix: dict[str, Any], rendered: str | None = None, tmp_path: Path | None = None
) -> list[str]:
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


def _row(matrix: dict[str, Any], requirement_id: str) -> dict[str, Any]:
    return next(row for row in matrix["requirements"] if row["requirement_id"] == requirement_id)


def test_real_matrix_passes_without_reading_project_gsd(tmp_path: Path) -> None:
    matrix = _load_real_matrix()

    errors = _errors(matrix, tmp_path=tmp_path)

    assert errors == []


def test_cli_passes_against_real_matrix_and_rendered_pair() -> None:
    exit_code = main(
        [
            "--matrix",
            str(REAL_MATRIX),
            "--rendered",
            str(REAL_RENDERED),
            "--require-active-scope",
            "--reject-unsafe-claims",
        ]
    )

    assert exit_code == 0


def test_cli_passes_with_real_coverage_handoff() -> None:
    exit_code = main(
        [
            "--matrix",
            str(REAL_MATRIX),
            "--rendered",
            str(REAL_RENDERED),
            "--coverage",
            str(REAL_COVERAGE),
            "--require-active-scope",
            "--reject-unsafe-claims",
        ]
    )

    assert exit_code == 0


def test_rejects_incomplete_coverage_handoff() -> None:
    errors = validate_coverage_markdown(
        "# S05 Coverage Handoff\n\nR040 only\n", required_requirements=REQUIRED_IDS
    )

    assert any(
        "coverage markdown missing marker: ## Q5 — Failure Modes" in error for error in errors
    )
    assert any("coverage markdown missing requirement id: R001" in error for error in errors)


def test_rejects_unsafe_true_marker_in_coverage_handoff() -> None:
    coverage = REAL_COVERAGE.read_text(encoding="utf-8") + "\nloader_implementation_claimed: true\n"

    errors = validate_coverage_markdown(coverage, required_requirements=REQUIRED_IDS)

    assert any("unsafe true boolean marker" in error for error in errors)


def test_cli_rejects_malformed_json(tmp_path: Path) -> None:
    malformed = tmp_path / "matrix.json"
    malformed.write_text("{not json", encoding="utf-8")

    exit_code = main(["--matrix", str(malformed), "--rendered", str(REAL_RENDERED)])

    assert exit_code == 2


def test_rejects_missing_required_requirement_id(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    matrix["requirements"] = [
        row for row in matrix["requirements"] if row["requirement_id"] != "R029"
    ]

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
    assert any(
        "R024 allowed_claims must be a non-empty list of strings" in error for error in errors
    )


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

    assert any(
        "R024 evidence path does not exist: .gsd/does/not/exist.md" in error for error in errors
    )


@pytest.mark.parametrize(
    ("requirement_id", "claim"),
    [
        ("R024", "M026 fully validates R024."),
        ("R027", "M026 validates graph readiness."),
        ("R029", "M026 authorizes KG import readiness."),
        ("R036", "M026 authorizes production LadybugDB writes."),
        ("R040", "M026 globally validates R040."),
        (
            "R050",
            "M026 implements R050 with a deterministic article structure artifact detection CLI.",
        ),
    ],
)
def test_rejects_unsafe_positive_claim_phrases(
    tmp_path: Path, requirement_id: str, claim: str
) -> None:
    matrix = _load_real_matrix()
    _row(matrix, requirement_id)["allowed_claims"].append(claim)

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("unsafe claim phrase" in error and requirement_id in error for error in errors)


@pytest.mark.parametrize(
    ("field", "expected"),
    [
        ("kg_import_or_readiness_claimed", "unsafe boolean field must not be true"),
        ("graph_validation_claimed", "unsafe boolean field must not be true"),
        ("production_ladybugdb_writes_claimed", "unsafe boolean field must not be true"),
        ("import_ready", "unsafe boolean field must not be true"),
        ("raw_payloads_embedded", "unsafe boolean field must not be true"),
    ],
)
def test_rejects_unsafe_true_boolean_fields(tmp_path: Path, field: str, expected: str) -> None:
    matrix = _load_real_matrix()
    if field in matrix["safety_flags"]:
        matrix["safety_flags"][field] = True
    else:
        _row(matrix, "R027")[field] = True

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any(expected in error and field in error for error in errors)


@pytest.mark.parametrize(
    "field",
    [
        "raw_article_text",
        "binary_payload",
        "base64_payload",
        "vector_payload",
        "secret_value",
        "production_connection",
    ],
)
def test_rejects_raw_binary_base64_vector_secret_or_production_field_names(
    tmp_path: Path, field: str
) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R030")[field] = "payload must not be stored in coverage artifacts"

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any(
        "contains unsafe raw/binary/base64/vector/secret field name" in error and field in error
        for error in errors
    )


@pytest.mark.parametrize(
    "requirement_id",
    [
        "R019",
        "R022",
        "R023",
        "R024",
        "R027",
        "R029",
        "R031",
        "R032",
        "R033",
        "R035",
        "R051",
        "R052",
    ],
)
def test_rejects_false_validation_of_broad_active_requirements(
    tmp_path: Path, requirement_id: str
) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, requirement_id)
    row["current_status"] = "validated"
    row["s05_verdict"] = "validated_by_m026"
    row["allowed_claims"].append(f"M026 fully validates {requirement_id}.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any(
        requirement_id in error and ("must" in error or "unsafe claim" in error) for error in errors
    )


def test_rejects_r030_reopened_or_newly_validated(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R030")
    row["current_status"] = "active"
    row["s05_verdict"] = "newly_validated_by_m026"
    row["allowed_claims"] = ["M026 newly validates R030."]

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("R030 current_status must be validated" in error for error in errors)
    assert any(
        "R030 current_status must remain validated existing context" in error for error in errors
    )
    assert any("R030 must not be claimed as newly validated" in error for error in errors)


def test_rejects_r040_globalized_beyond_this_milestone(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R040")
    row["current_status"] = "validated"
    row["s05_verdict"] = "fully_validated"
    row["allowed_claims"].append("M026 globally validates R040 for all future infrastructure.")

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("R040 current_status must be active" in error for error in errors)
    assert any(
        "R040 must be milestone-local followed-constraint evidence" in error for error in errors
    )


def test_rejects_r050_as_implemented_by_m026(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    row = _row(matrix, "R050")
    row["current_status"] = "validated"
    row["s05_verdict"] = "implemented_by_m026"
    row["allowed_claims"].append(
        "M026 implements R050 with a deterministic article structure artifact detection CLI."
    )

    errors = _errors(matrix, tmp_path=tmp_path)

    assert any("R050 current_status must be active" in error for error in errors)
    assert any("R050 must remain a future consumer" in error for error in errors)


def test_rejects_stale_rendered_markdown(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    rendered = _load_rendered().replace("R036", "RXXX")

    errors = _errors(matrix, rendered=rendered, tmp_path=tmp_path)

    assert any("rendered markdown missing requirement id: R036" in error for error in errors)


def test_rejects_rendered_markdown_missing_source_matrix_path(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    rendered = _load_rendered().replace(
        "doc/validation/m026_requirement_scope_matrix.json", "doc/validation/old.json"
    )

    errors = _errors(matrix, rendered=rendered, tmp_path=tmp_path)

    assert any(
        "rendered markdown does not reference the source matrix JSON path" in error
        for error in errors
    )


def test_cli_rejects_negative_fixture(tmp_path: Path) -> None:
    matrix = _load_real_matrix()
    _row(matrix, "R027")["allowed_claims"].append("M026 validates graph readiness.")
    matrix_path = tmp_path / "matrix.json"
    rendered_path = tmp_path / "rendered.md"
    _write_json(matrix_path, matrix)
    rendered_path.write_text(_load_rendered(), encoding="utf-8")

    exit_code = main(
        [
            "--matrix",
            str(matrix_path),
            "--rendered",
            str(rendered_path),
            "--require-active-scope",
            "--reject-unsafe-claims",
        ]
    )

    assert exit_code == 1
