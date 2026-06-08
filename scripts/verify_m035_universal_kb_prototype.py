#!/usr/bin/env python3
"""Verify the M035 Universal KB no-write prototype end to end.

This wrapper is intentionally local-only. It runs stable M034 ADR package checks,
all M035 Universal KB tests, lint for the M035 prototype surfaces, and then
inspects a fresh metadata-only rehearsal artifact set to prove graph write,
promotion, and production import flags remain false.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

ARTIFACT_DIR = ROOT / "artifacts" / "m035-universal-kb-prototype" / "rehearsal"
M034_PACKAGE_DIR = ROOT / ".gsd" / "milestones" / "M034-kuei9y" / "decision-package"

M034_PACKAGE_CHECKS = [
    [
        sys.executable,
        "scripts/verify_m034_formal_adr_package.py",
        "--package-dir",
        str(M034_PACKAGE_DIR),
    ],
    [
        sys.executable,
        "scripts/verify_m034_adr_template_and_north_star.py",
        "--package-dir",
        str(M034_PACKAGE_DIR),
    ],
]

M035_TESTS = [
    "tests/test_universal_kb_contracts.py",
    "tests/test_universal_kb_queue.py",
    "tests/test_universal_kb_sidecar_boundary.py",
    "tests/test_universal_kb_review_assistance.py",
    "tests/test_universal_kb_substrate_rehearsal.py",
    "tests/test_universal_kb_architecture_guards.py",
    "tests/test_universal_kb_rehearsal.py",
    "tests/test_minimax_structured.py",
    "tests/test_hybrid_retrieval.py",
    "tests/test_graph_readiness_review.py",
]

M035_RUFF_TARGETS = [
    "src/arxiv_archive/universal_kb_contracts.py",
    "src/arxiv_archive/universal_kb_queue.py",
    "src/arxiv_archive/universal_kb_sidecar_boundary.py",
    "src/arxiv_archive/universal_kb_review_assistance.py",
    "src/arxiv_archive/universal_kb_substrate_rehearsal.py",
    "src/arxiv_archive/universal_kb_rehearsal.py",
    "src/arxiv_archive/minimax_structured.py",
    "src/arxiv_archive/summarizer.py",
    "tests/test_universal_kb_contracts.py",
    "tests/test_universal_kb_queue.py",
    "tests/test_universal_kb_sidecar_boundary.py",
    "tests/test_universal_kb_review_assistance.py",
    "tests/test_universal_kb_substrate_rehearsal.py",
    "tests/test_universal_kb_architecture_guards.py",
    "tests/test_universal_kb_rehearsal.py",
    "tests/test_minimax_structured.py",
]

EXPECTED_ARTIFACTS = [
    "candidate.json",
    "review_packet.json",
    "review_trace.json",
    "queue_inspect.json",
    "readiness_handoff.json",
    "summary.json",
]

FORBIDDEN_TRUE_FLAGS = [
    "graph_write_allowed",
    "promotion_allowed",
    "production_import_attempted",
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "import_eligible",
    "minimax_source_of_truth",
    "raw_prompt_persisted",
    "credential_value_logged",
]

FORBIDDEN_PAYLOAD_TERMS = [
    "api_key",
    "secret_value",
    "embedding_payload",
    "vector_payload",
    "chunk_text_payload",
    "paper_text_payload",
    "claim_text_payload",
]


def emit(message: str) -> None:
    sys.stdout.write(f"{message}\n")


def run(command: list[str]) -> None:
    emit(f"$ {' '.join(command)}")
    subprocess.run(command, cwd=ROOT, check=True)


def clean_rehearsal_artifacts() -> None:
    ARTIFACT_DIR.mkdir(parents=True, exist_ok=True)
    for name in EXPECTED_ARTIFACTS:
        path = ARTIFACT_DIR / name
        if path.exists():
            path.unlink()
    clean_queue_support_files()
    pycache = ARTIFACT_DIR / "__pycache__"
    if pycache.exists():
        shutil.rmtree(pycache)


def clean_queue_support_files() -> None:
    for suffix in ("", "-wal", "-shm"):
        path = ARTIFACT_DIR / f"queue.sqlite{suffix}"
        if path.exists():
            path.unlink()


def load_json(name: str) -> dict[str, Any]:
    path = ARTIFACT_DIR / name
    if not path.exists():
        raise AssertionError(f"missing rehearsal artifact: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def assert_false(payload: dict[str, Any], key: str, *, label: str) -> None:
    if payload.get(key) is not False:
        raise AssertionError(f"{label}.{key} must be false")


def assert_safety_flags_false(payload: dict[str, Any], *, label: str) -> None:
    flags = payload.get("safety_flags")
    if not isinstance(flags, dict):
        raise AssertionError(f"{label}.safety_flags must be an object")
    for key in ("graph_import_allowed", "graphdb_written", "ladybugdb_written", "production_import_attempted", "import_eligible"):
        if flags.get(key) is not False:
            raise AssertionError(f"{label}.safety_flags.{key} must be false")


def inspect_rehearsal_artifacts() -> None:
    from arxiv_archive.minimax_structured import DEFAULT_MINIMAX_MODEL
    from arxiv_archive.universal_kb_rehearsal import run_universal_kb_no_write_rehearsal

    clean_rehearsal_artifacts()
    result = run_universal_kb_no_write_rehearsal(ARTIFACT_DIR)
    if result.model != "MiniMax-M3-512k" or DEFAULT_MINIMAX_MODEL != "MiniMax-M3-512k":
        raise AssertionError("MiniMax-M3-512k must remain the helper default")

    candidate = load_json("candidate.json")
    review_trace = load_json("review_trace.json")
    queue_inspect = load_json("queue_inspect.json")
    handoff = load_json("readiness_handoff.json")
    summary = load_json("summary.json")

    assert_safety_flags_false(candidate, label="candidate")
    assert_safety_flags_false(handoff, label="handoff")
    for key in ("graph_write_allowed", "promotion_allowed", "production_import_attempted"):
        assert_false(handoff, key, label="handoff")
        assert_false(summary, key, label="summary")
    for key in ("minimax_source_of_truth", "raw_prompt_persisted", "credential_value_logged"):
        assert_false(review_trace, key, label="review_trace")
    if review_trace.get("helper_evidence_only") is not True:
        raise AssertionError("review_trace.helper_evidence_only must be true")
    if queue_inspect.get("job", {}).get("status") != "ready":
        raise AssertionError("queue job must be ready after dependency gates open")
    if queue_inspect.get("job", {}).get("tool_version") != "MiniMax-M3-512k":
        raise AssertionError("queue job must record MiniMax-M3-512k helper metadata")

    for name in EXPECTED_ARTIFACTS:
        text = (ARTIFACT_DIR / name).read_text(encoding="utf-8").lower()
        for term in FORBIDDEN_PAYLOAD_TERMS:
            if term in text:
                raise AssertionError(f"{name} contains forbidden payload term {term!r}")

    emit("M035 rehearsal artifact inspection passed")
    emit(f"artifact_dir={ARTIFACT_DIR.relative_to(ROOT)}")
    emit(f"candidate_id={result.candidate_id}")
    emit("graph_write_allowed=false promotion_allowed=false production_import_attempted=false")
    clean_queue_support_files()


def main() -> int:
    for command in M034_PACKAGE_CHECKS:
        run(command)
    run(["uv", "run", "pytest", *M035_TESTS, "-q"])
    run(["uv", "run", "ruff", "check", *M035_RUFF_TARGETS])
    inspect_rehearsal_artifacts()
    emit("M035 Universal KB prototype verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
