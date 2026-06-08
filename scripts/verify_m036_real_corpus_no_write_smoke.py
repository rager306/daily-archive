#!/usr/bin/env python3
"""Verify the M036 real-corpus no-write smoke and continuity audit."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_DIR = ROOT / "artifacts" / "m036-real-corpus-no-write-smoke"
MANIFEST = ARTIFACT_DIR / "manifest.json"
RUN_DIR = ARTIFACT_DIR / "run"
AUDIT_JSON = ARTIFACT_DIR / "audit.json"
AUDIT_MD = ARTIFACT_DIR / "audit.md"

FORBIDDEN_PAYLOAD_TERMS = (
    "api_key",
    "secret_value",
    "bearer ",
    "x-api-key",
    "embedding_payload",
    "vector_payload",
    "chunk_text_payload",
    "paper_text_payload",
    "claim_text_payload",
)


def emit(message: str) -> None:
    sys.stdout.write(f"{message}\n")


def run(command: list[str]) -> None:
    emit(f"$ {' '.join(command)}")
    subprocess.run(command, cwd=ROOT, check=True)


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def assert_false(payload: dict[str, Any], key: str, label: str) -> None:
    if payload.get(key) is not False:
        raise AssertionError(f"{label}.{key} must be false")


def inspect_outputs() -> None:
    summary = load_json(RUN_DIR / "summary.json")
    audit = load_json(AUDIT_JSON)
    if summary.get("article_count") != 5 or summary.get("completed_handoff_count") != 5:
        raise AssertionError("M036 smoke must process 5 articles and complete 5 handoffs")
    if audit.get("article_count") != 5 or audit.get("completed_handoff_count") != 5:
        raise AssertionError("M036 audit must cover 5 articles and 5 handoffs")
    for key in ("graph_write_allowed", "promotion_allowed", "production_import_attempted", "import_eligible"):
        assert_false(summary, key, "summary")
        assert_false(audit["safety"], key, "audit.safety")
    blockers = set(audit.get("blockers_for_import", []))
    expected_blockers = {"missing_loader_evidence", "legacy_or_missing_article_safety_flags"}
    if not expected_blockers.issubset(blockers):
        raise AssertionError(f"expected continuity blockers not reported: {expected_blockers - blockers}")
    json_files = [path for path in ARTIFACT_DIR.rglob("*.json") if path.is_file()]
    for path in json_files:
        text = path.read_text(encoding="utf-8", errors="ignore").lower()
        for term in FORBIDDEN_PAYLOAD_TERMS:
            if term in text:
                raise AssertionError(f"{path} contains forbidden payload term {term!r}")
    emit(f"m036_json_artifacts_scanned={len(json_files)}")
    emit("graph_write_allowed=false promotion_allowed=false production_import_attempted=false import_eligible=false")


def main() -> int:
    run([sys.executable, "scripts/verify_m035_universal_kb_prototype.py"])
    run([
        "uv",
        "run",
        "pytest",
        "tests/test_m036_real_corpus_no_write_smoke.py",
        "tests/test_m036_real_corpus_smoke_audit.py",
        "-q",
    ])
    run([
        "uv",
        "run",
        "ruff",
        "check",
        "scripts/select_m036_real_corpus_smoke_batch.py",
        "scripts/run_m036_real_corpus_no_write_smoke.py",
        "scripts/audit_m036_real_corpus_smoke.py",
        "scripts/verify_m036_real_corpus_no_write_smoke.py",
        "tests/test_m036_real_corpus_no_write_smoke.py",
        "tests/test_m036_real_corpus_smoke_audit.py",
    ])
    run([
        "uv",
        "run",
        "python",
        "scripts/select_m036_real_corpus_smoke_batch.py",
        "--limit",
        "5",
        "--output",
        str(MANIFEST.relative_to(ROOT)),
    ])
    run([
        "uv",
        "run",
        "python",
        "scripts/run_m036_real_corpus_no_write_smoke.py",
        "--manifest",
        str(MANIFEST.relative_to(ROOT)),
        "--output-dir",
        str(RUN_DIR.relative_to(ROOT)),
        "--clean",
    ])
    run([
        "uv",
        "run",
        "python",
        "scripts/audit_m036_real_corpus_smoke.py",
        "--manifest",
        str(MANIFEST.relative_to(ROOT)),
        "--run-dir",
        str(RUN_DIR.relative_to(ROOT)),
        "--output-json",
        str(AUDIT_JSON.relative_to(ROOT)),
        "--output-md",
        str(AUDIT_MD.relative_to(ROOT)),
    ])
    inspect_outputs()
    emit("M036 real-corpus no-write smoke verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
