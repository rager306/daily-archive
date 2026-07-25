#!/usr/bin/env python3
"""Final verifier for the M034 decision package."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

PACKAGE_FILES = [
    "ADR-TEMPLATE.md",
    "ADR-INDEX.md",
    "ADR-000-universal-kb-north-star.md",
    "ADR-002-defer-final-graphdb-selection.md",
    "ADR-003-durable-lazy-async-evidence-pipeline.md",
    "ADR-004-sidecars-as-candidate-evidence-producers.md",
    "ADR-005-no-direct-extractor-to-graphdb-path.md",
    "ADR-006-agent-boundary.md",
    "ADR-007-quantmind-pattern-source-not-runtime-dependency.md",
    "PRD.md",
    "FUNCTIONAL-REQUIREMENTS.md",
    "NON-FUNCTIONAL-REQUIREMENTS.md",
    "CONTRACTS.md",
    "SAFETY-INVARIANTS.md",
    "STATUS-MATRIX.md",
    "FAILURE-TAXONOMY.md",
    "ARTIFACT-DEPENDENCY-MODEL.md",
    "ROADMAP-GATES.md",
    "CONFLICT-RESOLUTION-PLAN.md",
    "OPEN-QUESTIONS.md",
    "NEXT-MILESTONE-HANDOFF.md",
    "DECISION-PACKAGE-SUMMARY.md",
]
SAFETY_MARKERS = [
    "graph_import_allowed=false",
    "graphdb_written=false",
    "ladybugdb_written=false",
    "production_import_attempted=false",
    "import_eligible=false",
]


def run(cmd: list[str]) -> tuple[int, str, str]:
    proc = subprocess.run(cmd, text=True, capture_output=True, check=False)
    return proc.returncode, proc.stdout, proc.stderr


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-dir", required=True, type=Path)
    parser.add_argument("--requirements", required=True, type=Path)
    parser.add_argument("--decisions", required=True, type=Path)
    args = parser.parse_args()

    failures: list[str] = []
    pkg: Path = args.package_dir
    for name in PACKAGE_FILES:
        if not (pkg / name).exists():
            failures.append(f"missing package file: {name}")

    summary = (
        (pkg / "DECISION-PACKAGE-SUMMARY.md").read_text(encoding="utf-8")
        if (pkg / "DECISION-PACKAGE-SUMMARY.md").exists()
        else ""
    )
    for marker in [
        "local-first universal knowledge base",
        "scientific articles",
        "ADR-002",
        "Durable Evidence Pipeline Prototype Planning",
    ]:
        if marker not in summary:
            failures.append(f"summary missing marker: {marker}")
    for marker in SAFETY_MARKERS:
        if marker not in summary:
            failures.append(f"summary missing safety marker: {marker}")

    verifier_cmds = [
        [
            sys.executable,
            "scripts/verify_m034_rd_consistency_audit.py",
            "--package-dir",
            str(pkg),
            "--requirements",
            str(args.requirements),
            "--decisions",
            str(args.decisions),
        ],
        [
            sys.executable,
            "scripts/verify_m034_adr_template_and_north_star.py",
            "--package-dir",
            str(pkg),
        ],
        [sys.executable, "scripts/verify_m034_formal_adr_package.py", "--package-dir", str(pkg)],
        [sys.executable, "scripts/verify_m034_prd_requirements.py", "--package-dir", str(pkg)],
        [sys.executable, "scripts/verify_m034_contracts_invariants.py", "--package-dir", str(pkg)],
        [sys.executable, "scripts/verify_m034_roadmap_gates.py", "--package-dir", str(pkg)],
    ]
    combined_stdout: list[str] = []
    for cmd in verifier_cmds:
        code, stdout, stderr = run(cmd)
        combined_stdout.append(stdout.strip())
        if code != 0:
            failures.append(f"sub-verifier failed: {' '.join(cmd)}\n{stderr}")

    if failures:
        sys.stderr.write("M034 decision package verification failed:\n")
        for failure in failures:
            sys.stderr.write(f"- {failure}\n")
        return 1

    sys.stdout.write("M034 decision package verification passed\n")
    sys.stdout.write(f"package_files={len(PACKAGE_FILES)}\n")
    sys.stdout.write("sub_verifiers=6\n")
    for line in combined_stdout:
        if line:
            sys.stdout.write(f"---\n{line}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
