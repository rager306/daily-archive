from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

# pyrefly: ignore [missing-import]
import scripts.verify_m028_smoke_closeout as verifier

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_DIR = (
    ROOT / "data/article_corpora/m028-universal-loader-runtime-smoke-v1/smoke-replay-closeout"
)
SUMMARY_PATH = FIXTURE_DIR / "smoke-replay-closeout-summary.json"
EVENTS_PATH = FIXTURE_DIR / "smoke-replay-closeout-events.jsonl"
REPORT_PATH = FIXTURE_DIR / "smoke-replay-closeout-report.md"


def load_fixture() -> tuple[dict, list[dict], str]:
    summary = json.loads(SUMMARY_PATH.read_text(encoding="utf-8"))
    events = [
        json.loads(line)
        for line in EVENTS_PATH.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    report = REPORT_PATH.read_text(encoding="utf-8")
    return summary, events, report


def write_fixture(
    tmp_path: Path, summary: dict, events: list[dict], report: str
) -> tuple[Path, Path, Path]:
    summary_path = tmp_path / "summary.json"
    events_path = tmp_path / "events.jsonl"
    report_path = tmp_path / "report.md"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    events_path.write_text(
        "\n".join(json.dumps(event, sort_keys=True) for event in events) + "\n", encoding="utf-8"
    )
    report_path.write_text(report, encoding="utf-8")
    return summary_path, events_path, report_path


def run_validation(tmp_path: Path, summary: dict, events: list[dict], report: str) -> list[str]:
    summary_path, events_path, report_path = write_fixture(tmp_path, summary, events, report)
    diagnostics = verifier.validate_closeout(
        summary_path, events_path, report_path, reject_unsafe_claims=True
    )
    return [diagnostic.code for diagnostic in diagnostics]


def test_validate_closeout_accepts_current_fixture() -> None:
    diagnostics = verifier.validate_closeout(
        SUMMARY_PATH, EVENTS_PATH, REPORT_PATH, reject_unsafe_claims=True
    )
    assert diagnostics == []


def test_validate_closeout_accepts_planned_smoke_prefixed_filenames() -> None:
    diagnostics = verifier.validate_closeout(
        FIXTURE_DIR / "smoke-replay-closeout-summary.json",
        FIXTURE_DIR / "smoke-replay-closeout-events.jsonl",
        FIXTURE_DIR / "smoke-replay-closeout-report.md",
        reject_unsafe_claims=True,
    )
    assert diagnostics == []


def test_rejects_stale_14_ref_scope(tmp_path: Path) -> None:
    summary, events, report = load_fixture()
    summary["source_acquisition_preflight"]["url_ref_count"] = 14
    codes = run_validation(tmp_path, summary, events, report)
    assert "STALE_14_REF_SCOPE" in codes
    assert "URL_REF_COUNT_MISMATCH" in codes


def test_rejects_missing_verify_stage_event(tmp_path: Path) -> None:
    summary, events, report = load_fixture()
    missing_verify = [
        event
        for event in events
        if event["stage"] != "S04_verify_universal_loader_evidence_bundles"
    ]
    summary["stage_events"] = copy.deepcopy(missing_verify)
    codes = run_validation(tmp_path, summary, missing_verify, report)
    assert "STAGE_ORDER_MISMATCH" in codes
    assert "EVENT_STAGE_ORDER_MISMATCH" in codes


def test_rejects_checksum_drift(tmp_path: Path) -> None:
    summary, events, report = load_fixture()
    summary["source_acquisition_preflight"]["selection"]["sha256"] = hashlib.sha256(
        b"drift"
    ).hexdigest()
    codes = run_validation(tmp_path, summary, events, report)
    assert "ARTIFACT_SHA256_MISMATCH" in codes


def test_rejects_true_unsafe_flag(tmp_path: Path) -> None:
    summary, events, report = load_fixture()
    summary["safety_flags"]["ladybugdb_written"] = True
    codes = run_validation(tmp_path, summary, events, report)
    assert "UNSAFE_FLAG_TRUE" in codes


def test_rejects_raw_payload_marker_in_serialized_values(tmp_path: Path) -> None:
    summary, events, report = load_fixture()
    summary["diagnostic_note"] = "raw_article_text=leaked body"
    codes = run_validation(tmp_path, summary, events, report)
    assert "RAW_PAYLOAD_MARKER" in codes


def test_rejects_payload_bearing_key(tmp_path: Path) -> None:
    summary, events, report = load_fixture()
    summary["raw_pdf_bytes"] = "not allowed even if redacted"
    codes = run_validation(tmp_path, summary, events, report)
    assert "PAYLOAD_KEY_FORBIDDEN" in codes


def test_rejects_kg_parser_readiness_wording(tmp_path: Path) -> None:
    summary, events, report = load_fixture()
    report = report + "\nKG readiness: true\nparser_ready=true\n"
    codes = run_validation(tmp_path, summary, events, report)
    assert "UNSAFE_CLAIM_WORDING" in codes


def test_rejects_malformed_jsonl(tmp_path: Path) -> None:
    summary, _events, report = load_fixture()
    summary_path = tmp_path / "summary.json"
    events_path = tmp_path / "events.jsonl"
    report_path = tmp_path / "report.md"
    summary_path.write_text(json.dumps(summary), encoding="utf-8")
    events_path.write_text('{"stage": "ok"}\nnot-json\n', encoding="utf-8")
    report_path.write_text(report, encoding="utf-8")
    diagnostics = verifier.validate_closeout(
        summary_path, events_path, report_path, reject_unsafe_claims=True
    )
    assert "JSONL_MALFORMED" in [diagnostic.code for diagnostic in diagnostics]


def test_rejects_absolute_artifact_path(tmp_path: Path) -> None:
    summary, events, report = load_fixture()
    summary["source_acquisition_preflight"]["selection"]["path"] = str(SUMMARY_PATH)
    codes = run_validation(tmp_path, summary, events, report)
    assert "ARTIFACT_PATH_UNSAFE" in codes
