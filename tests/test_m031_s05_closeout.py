from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

CORPUS_DIR = Path("data/article_corpora/m031-catalog-backed-replay-v1")
SCRIPT = Path("scripts/verify_m031_s05_closeout.py")
S04_CLOSEOUT = CORPUS_DIR / "chunk-evidence-closeout-summary.json"
IMPORT_SUMMARY = CORPUS_DIR / "import-boundary-rehearsal" / "import-boundary-summary.json"
IMPORT_DIAGNOSTICS = CORPUS_DIR / "import-boundary-rehearsal" / "import-boundary-diagnostics.jsonl"
MATRIX_JSON = CORPUS_DIR / "progression-matrix.json"
AUDIT_JSON = CORPUS_DIR / "m031-continuity-audit.json"
REVIEW_EVENTS = CORPUS_DIR / "chunk-evidence" / "independent-review-events.jsonl"


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


def test_s05_closeout_cli_writes_passing_summary_diagnostics_and_report(tmp_path: Path) -> None:
    summary_path = tmp_path / "s05-closeout-summary.json"
    diagnostics_path = tmp_path / "s05-closeout-diagnostics.jsonl"
    report_path = tmp_path / "s05-closeout-report.md"

    result = _run(
        [
            "--summary-out",
            summary_path.as_posix(),
            "--diagnostics-out",
            diagnostics_path.as_posix(),
            "--report-out",
            report_path.as_posix(),
        ]
    )

    assert result.returncode == 0, result.stderr
    summary = _read_json(summary_path)
    diagnostics = [
        line for line in diagnostics_path.read_text(encoding="utf-8").splitlines() if line.strip()
    ]
    report = report_path.read_text(encoding="utf-8")

    assert summary["status"] == "passed"
    assert summary["failure_count"] == 0
    assert summary["progression_row_count"] == 7
    assert summary["rejected_import_candidate_count"] == 7
    assert summary["accepted_count"] == 0
    assert summary["import_eligible_count"] == 0
    assert summary["independent_review_completed_count"] == 0
    assert summary["completed_review_refusal_in_force"] is True
    assert summary["network_fetch_attempted"] is False
    assert summary["model_call_attempted"] is False
    assert summary["graph_import_allowed"] is False
    assert summary["trusted_kg_import_allowed"] is False
    assert summary["ladybugdb_written"] is False
    assert diagnostics == []
    assert "## Recovery Commands" in report
    assert "uv run python scripts/verify_m031_s05_closeout.py" in report
    assert "accepted/import-eligible candidates: 0/0" in report
    assert "normalized_markdown" not in repr(summary) + report


def test_s05_closeout_fails_closed_before_writes_on_permissive_import_summary(
    tmp_path: Path,
) -> None:
    import_summary = _read_json(IMPORT_SUMMARY)
    import_summary["accepted_count"] = 1
    bad_import_summary = _write_json(tmp_path / "bad-import-summary.json", import_summary)
    summary_path = tmp_path / "out" / "s05-closeout-summary.json"
    diagnostics_path = tmp_path / "out" / "s05-closeout-diagnostics.jsonl"
    report_path = tmp_path / "out" / "s05-closeout-report.md"

    result = _run(
        [
            "--import-summary",
            bad_import_summary.as_posix(),
            "--summary-out",
            summary_path.as_posix(),
            "--diagnostics-out",
            diagnostics_path.as_posix(),
            "--report-out",
            report_path.as_posix(),
        ]
    )

    assert result.returncode == 1
    assert "M031_S05_IMPORT_BOUNDARY_PERMISSIVE" in result.stderr
    assert not summary_path.exists()
    assert not diagnostics_path.exists()
    assert not report_path.exists()


def test_s05_closeout_rejects_missing_progression_row(tmp_path: Path) -> None:
    matrix = _read_json(MATRIX_JSON)
    matrix["rows"] = matrix["rows"][:-1]  # type: ignore[index]
    matrix["row_count"] = 6
    bad_matrix = _write_json(tmp_path / "bad-progression-matrix.json", matrix)

    result = _run(
        [
            "--matrix-json",
            bad_matrix.as_posix(),
            "--summary-out",
            (tmp_path / "summary.json").as_posix(),
        ]
    )

    assert result.returncode == 1
    assert "M031_S05_PROGRESSion_ROW_COUNT".upper() in result.stderr.upper()
    assert not (tmp_path / "summary.json").exists()


def test_s05_closeout_rejects_completed_review_claim_without_verdict(tmp_path: Path) -> None:
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

    result = _run(
        [
            "--review-events",
            events_path.as_posix(),
            "--summary-out",
            (tmp_path / "summary.json").as_posix(),
        ]
    )

    assert result.returncode == 1
    assert "M031_S05_COMPLETED_REVIEW_WITHOUT_VERDICT" in result.stderr
    assert not (tmp_path / "summary.json").exists()


def test_s05_closeout_rejects_raw_payload_leakage(tmp_path: Path) -> None:
    audit = _read_json(AUDIT_JSON)
    audit["raw_text"] = "Local Parser Ready Paper"
    bad_audit = _write_json(tmp_path / "bad-audit.json", audit)

    result = _run(
        [
            "--audit-json",
            bad_audit.as_posix(),
            "--summary-out",
            (tmp_path / "summary.json").as_posix(),
        ]
    )

    assert result.returncode == 1
    assert "M031_S05_RAW_PAYLOAD_LEAKAGE" in result.stderr
    assert not (tmp_path / "summary.json").exists()
