from __future__ import annotations

import copy
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORPUS_DIR = ROOT / "data" / "article_corpora" / "m028-universal-loader-runtime-smoke-v1"
CORPUS_REL = CORPUS_DIR.relative_to(ROOT).as_posix()
CANONICAL_CLOSEOUT_DIR = CORPUS_DIR / "smoke-replay-closeout"
CANONICAL_HASH_PATHS = [
    CORPUS_DIR / "selection.json",
    CORPUS_DIR / "source-acquisition-events.jsonl",
    CORPUS_DIR / "source-acquisition-summary.json",
    CORPUS_DIR / "acquisition-report.md",
    CANONICAL_CLOSEOUT_DIR / "smoke-replay-closeout-summary.json",
    CANONICAL_CLOSEOUT_DIR / "smoke-replay-closeout-events.jsonl",
    CANONICAL_CLOSEOUT_DIR / "smoke-replay-closeout-report.md",
]
EXPECTED_STAGES = [
    "S02_build_source_metadata_adapters",
    "S02_verify_source_metadata_adapters",
    "S03_build_pdf_acquisition_diagnostics",
    "S03_verify_pdf_acquisition_diagnostics",
    "S04_build_universal_loader_evidence_bundles",
    "S04_verify_universal_loader_evidence_bundles",
    "S05_build_hermes_digest_projection",
    "S05_verify_hermes_digest_projection",
]


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _canonical_hashes() -> dict[str, str]:
    return {path.relative_to(ROOT).as_posix(): _sha256(path) for path in CANONICAL_HASH_PATHS}


def _read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _read_jsonl(path: Path) -> list[dict[str, object]]:
    return [
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()
    ]


def _write_closeout(
    tmp_path: Path, summary: dict[str, object], events: list[dict[str, object]], report: str
) -> tuple[Path, Path, Path]:
    summary_path = tmp_path / "smoke-replay-closeout-summary.json"
    events_path = tmp_path / "smoke-replay-closeout-events.jsonl"
    report_path = tmp_path / "smoke-replay-closeout-report.md"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    events_path.write_text(
        "\n".join(json.dumps(event, sort_keys=True) for event in events) + "\n", encoding="utf-8"
    )
    report_path.write_text(report, encoding="utf-8")
    return summary_path, events_path, report_path


def _load_canonical_closeout() -> tuple[dict[str, object], list[dict[str, object]], str]:
    return (
        _read_json(CANONICAL_CLOSEOUT_DIR / "smoke-replay-closeout-summary.json"),
        _read_jsonl(CANONICAL_CLOSEOUT_DIR / "smoke-replay-closeout-events.jsonl"),
        (CANONICAL_CLOSEOUT_DIR / "smoke-replay-closeout-report.md").read_text(encoding="utf-8"),
    )


def _run_verifier(
    summary_path: Path, events_path: Path, report_path: Path
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "scripts/verify_m028_smoke_closeout.py",
            "--summary",
            str(
                summary_path.relative_to(ROOT)
                if summary_path.is_relative_to(ROOT)
                else summary_path
            ),
            "--events",
            str(events_path.relative_to(ROOT) if events_path.is_relative_to(ROOT) else events_path),
            "--report",
            str(report_path.relative_to(ROOT) if report_path.is_relative_to(ROOT) else report_path),
            "--reject-unsafe-claims",
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def _assert_verifier_rejects(
    tmp_path: Path,
    summary: dict[str, object],
    events: list[dict[str, object]],
    report: str,
    expected_codes: set[str],
) -> None:
    summary_path, events_path, report_path = _write_closeout(tmp_path, summary, events, report)
    completed = _run_verifier(summary_path, events_path, report_path)
    assert completed.returncode != 0, completed.stdout
    for code in expected_codes:
        assert code in completed.stderr
    assert "m028_smoke_closeout_verdict=fail" in completed.stderr


def test_closeout_runner_and_verifier_accept_real_corpus_without_mutating_canonical_artifacts(
    tmp_path: Path,
) -> None:
    before_hashes = _canonical_hashes()
    out_rel = f"{CORPUS_REL}/pytest-smoke-replay-closeout-{tmp_path.name}"
    out_dir = ROOT / out_rel
    shutil.rmtree(out_dir, ignore_errors=True)
    env = {**os.environ, "PYTHON": "python"}

    try:
        replay = subprocess.run(
            [
                sys.executable,
                "scripts/replay_m028_smoke_closeout.py",
                "--corpus-dir",
                CORPUS_REL,
                "--out-dir",
                out_rel,
                "--timeout-seconds",
                "120",
            ],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )
        assert replay.returncode == 0, replay.stderr

        summary_path = out_dir / "smoke-replay-closeout-summary.json"
        events_path = out_dir / "smoke-replay-closeout-events.jsonl"
        report_path = out_dir / "smoke-replay-closeout-report.md"
        verify = _run_verifier(summary_path, events_path, report_path)
        assert verify.returncode == 0, verify.stderr
        assert "m028_smoke_closeout_verdict=pass diagnostics=0" in verify.stdout

        summary = _read_json(summary_path)
        events = _read_jsonl(events_path)
        preflight = summary["source_acquisition_preflight"]
        assert summary["status"] == "pass"
        assert preflight["url_ref_count"] == 21  # pyrefly: ignore[bad-assignment]
        assert preflight["normalized_identity_count"] == 20  # pyrefly: ignore[bad-assignment]
        assert preflight["expansion_refs"] == ["R15", "R16", "R17", "R18", "R19", "R20", "R21"]  # pyrefly: ignore[bad-assignment]
        assert [event["stage"] for event in events] == EXPECTED_STAGES
        assert summary["stage_events"] == events
        assert summary["diagnostics"] == []
        assert all(event["status"] == "pass" and event["exit_code"] == 0 for event in events)
    finally:
        shutil.rmtree(out_dir, ignore_errors=True)

    assert _canonical_hashes() == before_hashes


def test_verifier_rejects_stale_14_ref_scope(tmp_path: Path) -> None:
    summary, events, report = _load_canonical_closeout()
    preflight = summary["source_acquisition_preflight"]
    assert isinstance(preflight, dict)
    preflight["url_ref_count"] = 14
    _assert_verifier_rejects(
        tmp_path, summary, events, report, {"STALE_14_REF_SCOPE", "URL_REF_COUNT_MISMATCH"}
    )


def test_verifier_rejects_stage_output_hash_drift(tmp_path: Path) -> None:
    summary, events, report = _load_canonical_closeout()
    mutated_events = copy.deepcopy(events)
    mutated_events[0]["output_hashes"][0]["sha256"] = hashlib.sha256(b"drift").hexdigest()  # pyrefly: ignore[bad-assignment]
    summary["stage_events"] = copy.deepcopy(mutated_events)
    _assert_verifier_rejects(
        tmp_path, summary, mutated_events, report, {"ARTIFACT_SHA256_MISMATCH"}
    )


def test_verifier_rejects_missing_stage_event(tmp_path: Path) -> None:
    summary, events, report = _load_canonical_closeout()
    mutated_events = [
        event
        for event in events
        if event["stage"] != "S04_verify_universal_loader_evidence_bundles"
    ]
    summary["stage_events"] = copy.deepcopy(mutated_events)
    _assert_verifier_rejects(
        tmp_path,
        summary,
        mutated_events,
        report,
        {"STAGE_ORDER_MISMATCH", "EVENT_STAGE_ORDER_MISMATCH"},
    )


def test_verifier_rejects_nonzero_unsafe_counter_and_flag(tmp_path: Path) -> None:
    summary, events, report = _load_canonical_closeout()
    summary["safety_flags"]["ladybugdb_written"] = True
    summary["unsafe_counters"]["import_eligible_count"] = 1
    _assert_verifier_rejects(
        tmp_path, summary, events, report, {"UNSAFE_FLAG_TRUE", "UNSAFE_COUNTER_NONZERO"}
    )


def test_verifier_rejects_raw_payload_marker(tmp_path: Path) -> None:
    summary, events, report = _load_canonical_closeout()
    summary["diagnostic_note"] = "raw_article_text=leaked body text"
    _assert_verifier_rejects(tmp_path, summary, events, report, {"RAW_PAYLOAD_MARKER"})


def test_verifier_rejects_payload_bearing_key(tmp_path: Path) -> None:
    summary, events, report = _load_canonical_closeout()
    summary["raw_pdf_bytes"] = "redacted"
    _assert_verifier_rejects(tmp_path, summary, events, report, {"PAYLOAD_KEY_FORBIDDEN"})


def test_verifier_rejects_absolute_or_escaping_artifact_path(tmp_path: Path) -> None:
    summary, events, report = _load_canonical_closeout()
    preflight = summary["source_acquisition_preflight"]
    assert isinstance(preflight, dict)
    selection_record = preflight["selection"]
    assert isinstance(selection_record, dict)
    selection_record["path"] = "../selection.json"
    _assert_verifier_rejects(tmp_path, summary, events, report, {"ARTIFACT_PATH_UNSAFE"})


def test_verifier_rejects_parser_kg_graph_readiness_leakage(tmp_path: Path) -> None:
    summary, events, report = _load_canonical_closeout()
    leaking_report = report + "\nkg_ready=true\nparser_ready=true\ngraph_ready=true\n"
    _assert_verifier_rejects(
        tmp_path, summary, events, leaking_report, {"RAW_PAYLOAD_MARKER", "UNSAFE_CLAIM_WORDING"}
    )
