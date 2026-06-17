#!/usr/bin/env python3
"""Compatibility verifier for the M036 real-corpus no-write smoke.

Prefer the unified command surface for new work:

    uv run python -m research_graph.workflows.universal_kb.smoke verify --profile full
"""

from __future__ import annotations

import importlib
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
for path in (ROOT, SRC):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))


def smoke_api() -> Any:
    return importlib.import_module("research_graph.workflows.universal_kb.smoke")


def emit(message: str) -> None:
    sys.stdout.write(f"{message}\n")


def run(command: list[str]) -> None:
    emit(f"$ {' '.join(command)}")
    subprocess.run(command, cwd=ROOT, check=True)


def main() -> int:
    run([
        "uv",
        "run",
        "pytest",
        "tests/test_m036_real_corpus_no_write_smoke.py",
        "tests/test_m036_real_corpus_smoke_audit.py",
        "tests/test_universal_kb_smoke_cli.py",
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
        "src/arxiv_archive/universal_kb_smoke.py",
        "tests/test_m036_real_corpus_no_write_smoke.py",
        "tests/test_m036_real_corpus_smoke_audit.py",
        "tests/test_universal_kb_smoke_cli.py",
    ])
    api = smoke_api()
    paths = api.SmokePaths()
    result = api.run_all(limit=5, profile="full", paths=paths)
    verified = api.run_verify(profile="fast", paths=paths)
    api.print_result(verified)
    emit(f"m036_json_artifacts_scanned={verified['json_artifacts_scanned']}")
    emit("M036 real-corpus no-write smoke verification passed")
    if result["article_count"] != verified["article_count"]:
        raise AssertionError("run and verify article counts diverged")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
