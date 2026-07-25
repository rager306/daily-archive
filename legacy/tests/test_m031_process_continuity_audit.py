from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

CORPUS_DIR = Path("data/article_corpora/m031-catalog-backed-replay-v1")
SCRIPT = Path("scripts/verify_m031_process_continuity_audit.py")
MATRIX_JSON = CORPUS_DIR / "progression-matrix.json"
MATRIX_MD = CORPUS_DIR / "progression-matrix.md"
AUDIT_JSON = CORPUS_DIR / "m031-continuity-audit.json"
AUDIT_MD = CORPUS_DIR / "m031-continuity-audit.md"
REVIEW_EVENTS = CORPUS_DIR / "chunk-evidence" / "independent-review-events.jsonl"
IMPORT_DIAGNOSTICS = CORPUS_DIR / "import-boundary-rehearsal" / "import-boundary-diagnostics.jsonl"


def _run(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, SCRIPT.as_posix(), *args], check=False, capture_output=True, text=True
    )


def _read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _write_json(path: Path, payload: dict[str, object]) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def _copy_outputs(tmp_path: Path) -> tuple[Path, Path, Path, Path]:
    matrix_json = tmp_path / "progression-matrix.json"
    matrix_md = tmp_path / "progression-matrix.md"
    audit_json = tmp_path / "m031-continuity-audit.json"
    audit_md = tmp_path / "m031-continuity-audit.md"
    matrix_json.write_text(MATRIX_JSON.read_text(encoding="utf-8"), encoding="utf-8")
    matrix_md.write_text(MATRIX_MD.read_text(encoding="utf-8"), encoding="utf-8")
    audit_json.write_text(AUDIT_JSON.read_text(encoding="utf-8"), encoding="utf-8")
    audit_md.write_text(AUDIT_MD.read_text(encoding="utf-8"), encoding="utf-8")
    return matrix_json, matrix_md, audit_json, audit_md


def _validate_args(
    matrix_json: Path, matrix_md: Path, audit_json: Path, audit_md: Path
) -> list[str]:
    return [
        "--validate-only",
        "--matrix-json",
        matrix_json.as_posix(),
        "--matrix-md",
        matrix_md.as_posix(),
        "--audit-json",
        audit_json.as_posix(),
        "--audit-md",
        audit_md.as_posix(),
    ]


def _generation_output_args(tmp_path: Path) -> list[str]:
    return [
        "--matrix-json",
        (tmp_path / "generated" / "progression-matrix.json").as_posix(),
        "--matrix-md",
        (tmp_path / "generated" / "progression-matrix.md").as_posix(),
        "--audit-json",
        (tmp_path / "generated" / "m031-continuity-audit.json").as_posix(),
        "--audit-md",
        (tmp_path / "generated" / "m031-continuity-audit.md").as_posix(),
    ]


def test_m031_continuity_cli_validates_generated_artifacts() -> None:
    result = _run(["--validate-only"])

    assert result.returncode == 0, result.stderr
    assert "rows=7" in result.stdout
    assert "fail_closed=true" in result.stdout


def test_m031_continuity_cli_generates_matrix_and_audit_to_custom_paths(tmp_path: Path) -> None:
    result = _run(_generation_output_args(tmp_path))

    assert result.returncode == 0, result.stderr
    matrix = _read_json(tmp_path / "generated" / "progression-matrix.json")
    audit = _read_json(tmp_path / "generated" / "m031-continuity-audit.json")
    matrix_md = (tmp_path / "generated" / "progression-matrix.md").read_text(encoding="utf-8")
    audit_md = (tmp_path / "generated" / "m031-continuity-audit.md").read_text(encoding="utf-8")

    assert matrix["row_count"] == 7
    assert len(matrix["rows"]) == 7  # type: ignore[arg-type]  # ty:ignore[invalid-argument-type]
    assert audit["schema_version"] == "m031-process-continuity-audit.v1"
    assert "ok_for_graph" in matrix_md
    assert "trusted_graph" in audit_md
    assert "LadybugDB" in audit_md


def test_validate_only_rejects_missing_progression_row(tmp_path: Path) -> None:
    matrix_json, matrix_md, audit_json, audit_md = _copy_outputs(tmp_path)
    matrix = _read_json(matrix_json)
    matrix["rows"] = matrix["rows"][:-1]  # type: ignore[index]  # ty:ignore[not-subscriptable]
    matrix["row_count"] = 6
    _write_json(matrix_json, matrix)

    result = _run(_validate_args(matrix_json, matrix_md, audit_json, audit_md))

    assert result.returncode == 1
    assert "M031_CONTINUITY_ROW_COUNT" in result.stderr


def test_validate_only_rejects_missing_stage_evidence(tmp_path: Path) -> None:
    matrix_json, matrix_md, audit_json, audit_md = _copy_outputs(tmp_path)
    matrix = _read_json(matrix_json)
    del matrix["rows"][0]["stages"]["chunking"]  # type: ignore[index]  # ty:ignore[not-subscriptable]
    _write_json(matrix_json, matrix)

    result = _run(_validate_args(matrix_json, matrix_md, audit_json, audit_md))

    assert result.returncode == 1
    assert "M031_CONTINUITY_STAGE_EVIDENCE" in result.stderr
    assert "chunking" in result.stderr


def test_validate_only_rejects_permissive_fail_closed_flags(tmp_path: Path) -> None:
    matrix_json, matrix_md, audit_json, audit_md = _copy_outputs(tmp_path)
    matrix = _read_json(matrix_json)
    matrix["fail_closed_flags"]["graph_import_allowed"] = True  # type: ignore[index]  # ty:ignore[invalid-assignment]
    matrix["rows"][1]["fail_closed_flags"]["trusted_kg_import_allowed"] = True  # type: ignore[index]  # ty:ignore[not-subscriptable]
    _write_json(matrix_json, matrix)

    result = _run(_validate_args(matrix_json, matrix_md, audit_json, audit_md))

    assert result.returncode == 1
    assert "M031_UNSAFE_FAIL_CLOSED_FLAG" in result.stderr
    assert "graph_import_allowed" in result.stderr or "trusted_kg_import_allowed" in result.stderr


def test_validate_only_rejects_raw_payload_leakage(tmp_path: Path) -> None:
    matrix_json, matrix_md, audit_json, audit_md = _copy_outputs(tmp_path)
    matrix = _read_json(matrix_json)
    matrix["rows"][0]["raw_text"] = "Local Parser Ready Paper"  # type: ignore[index]  # ty:ignore[not-subscriptable]
    _write_json(matrix_json, matrix)

    result = _run(_validate_args(matrix_json, matrix_md, audit_json, audit_md))

    assert result.returncode == 1
    assert "M031_RAW_PAYLOAD_LEAKAGE" in result.stderr


def test_generation_rejects_missing_import_boundary_refusal_artifacts(tmp_path: Path) -> None:
    empty_diagnostics = tmp_path / "empty-import-diagnostics.jsonl"
    empty_diagnostics.write_text("", encoding="utf-8")

    result = _run(
        ["--import-diagnostics", empty_diagnostics.as_posix(), *_generation_output_args(tmp_path)]
    )

    assert result.returncode == 2
    assert "M031_IMPORT_BOUNDARY_REFUSAL_ARTIFACT_MISSING" in result.stderr
    assert not (tmp_path / "generated" / "progression-matrix.json").exists()


def test_generation_rejects_completed_review_claim_without_verdict_evidence(tmp_path: Path) -> None:
    events_path = tmp_path / "independent-review-events.jsonl"
    events = []
    for line in REVIEW_EVENTS.read_text(encoding="utf-8").splitlines():
        if line.strip():
            event = json.loads(line)
            if event.get("event") == "independent_review.requested":
                event["output_contract_completed"] = True
                event["independent_review_completed"] = True
            events.append(event)
    events_path.write_text(
        "".join(json.dumps(event, sort_keys=True) + "\n" for event in events), encoding="utf-8"
    )

    result = _run(["--review-events", events_path.as_posix(), *_generation_output_args(tmp_path)])

    assert result.returncode == 2
    assert "M031_COMPLETED_REVIEW_WITHOUT_VERDICT" in result.stderr
    assert not (tmp_path / "generated" / "m031-continuity-audit.json").exists()


def test_validate_only_rejects_report_missing_required_coverage(tmp_path: Path) -> None:
    matrix_json, matrix_md, audit_json, audit_md = _copy_outputs(tmp_path)
    audit_text = audit_md.read_text(encoding="utf-8").replace(
        "## Negative Tests", "## Removed Negative Section"
    )
    audit_md.write_text(audit_text, encoding="utf-8")

    result = _run(_validate_args(matrix_json, matrix_md, audit_json, audit_md))

    assert result.returncode == 1
    assert "M031_CONTINUITY_REPORT_COVERAGE" in result.stderr
    assert "Negative Tests" in result.stderr
