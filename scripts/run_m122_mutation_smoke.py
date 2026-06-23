#!/usr/bin/env python3
"""Deterministic mutation smoke checks for M122 application seams.

This is intentionally small and CI-friendly: each mutation changes one critical
invariant in-place, runs the focused tests that should catch it, then restores the
original file before continuing. A mutation is considered killed when pytest exits
non-zero. The script fails if any mutation survives or if a target text is absent.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class MutationSpec:
    """One deterministic source mutation and the tests expected to kill it."""

    name: str
    path: Path
    old: str
    new: str
    tests: tuple[str, ...]
    rationale: str


def mutation_specs() -> tuple[MutationSpec, ...]:
    """Return the scoped M122 mutants used by this smoke runner."""

    coverage = REPO_ROOT / "src" / "research_graph" / "application" / "corpus" / "coverage.py"
    graph = REPO_ROOT / "src" / "research_graph" / "application" / "graph" / "probe.py"
    property_tests = "tests/test_m122_property_mutation_guards.py"
    return (
        MutationSpec(
            name="coverage_succeeded_inverted",
            path=coverage,
            old="return self.parser_errors == 0",
            new="return self.parser_errors != 0",
            tests=("tests/test_corpus_coverage_use_case.py", property_tests),
            rationale="Coverage success must be false when parser errors are non-zero.",
        ),
        MutationSpec(
            name="coverage_source_backed_denominator_drops_skips",
            path=coverage,
            old="total=request.parser.completed + request.parser.skipped,",
            new="total=request.parser.completed,",
            tests=("tests/test_corpus_coverage_use_case.py", property_tests),
            rationale="Source-backed denominator must include completed plus metadata-only skipped records.",
        ),
        MutationSpec(
            name="graph_zero_chunk_count_rejected",
            path=graph,
            old="if article.chunk_count < 0",
            new="if article.chunk_count <= 0",
            tests=("tests/test_graph_probe_use_case.py", property_tests),
            rationale="Zero chunks are valid; only negative chunk counts should fail input validation.",
        ),
        MutationSpec(
            name="graph_total_catalog_records_drops_excluded",
            path=graph,
            old="+ len(request.excluded_records),",
            new="+ 0,",
            tests=("tests/test_graph_probe_use_case.py", property_tests),
            rationale="Graph total catalog records must include excluded metadata-only records.",
        ),
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    return parser.parse_args(argv)


def run_mutation_smoke(specs: tuple[MutationSpec, ...]) -> dict[str, Any]:
    """Run all mutation specs and return a JSON-serializable summary."""

    results: list[dict[str, Any]] = []
    for spec in specs:
        started = time.perf_counter()
        original = spec.path.read_text(encoding="utf-8")
        if spec.old not in original:
            results.append(
                {
                    "name": spec.name,
                    "status": "error",
                    "reason": "target_text_not_found",
                    "path": _display_path(spec.path),
                    "duration_ms": int((time.perf_counter() - started) * 1000),
                }
            )
            continue
        mutated = original.replace(spec.old, spec.new, 1)
        try:
            spec.path.write_text(mutated, encoding="utf-8")
            completed = subprocess.run(
                [sys.executable, "-m", "pytest", *spec.tests],
                cwd=REPO_ROOT,
                check=False,
                text=True,
                capture_output=True,
            )
        finally:
            spec.path.write_text(original, encoding="utf-8")
        killed = completed.returncode != 0
        results.append(
            {
                "name": spec.name,
                "status": "killed" if killed else "survived",
                "path": _display_path(spec.path),
                "tests": list(spec.tests),
                "exit_code": completed.returncode,
                "duration_ms": int((time.perf_counter() - started) * 1000),
                "rationale": spec.rationale,
                "stdout_tail": _tail(completed.stdout),
                "stderr_tail": _tail(completed.stderr),
            }
        )
    survived = [result for result in results if result["status"] == "survived"]
    errors = [result for result in results if result["status"] == "error"]
    return {
        "schema_version": "m122-mutation-smoke.v00.01",
        "mutation_count": len(results),
        "killed": len([result for result in results if result["status"] == "killed"]),
        "survived": len(survived),
        "errors": len(errors),
        "succeeded": not survived and not errors,
        "results": results,
    }


def _display_path(path: Path) -> str:
    try:
        return path.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def _tail(value: str, *, max_chars: int = 800) -> str:
    return value[-max_chars:] if len(value) > max_chars else value


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    summary = run_mutation_smoke(mutation_specs())
    if args.json:
        print(json.dumps(summary, indent=2))
    else:
        print(
            "mutation_smoke: "
            f"killed={summary['killed']} survived={summary['survived']} "
            f"errors={summary['errors']}"
        )
        for result in summary["results"]:
            print(f"- {result['name']}: {result['status']}")
    return 0 if summary["succeeded"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
