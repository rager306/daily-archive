#!/usr/bin/env python3
"""Run M122 pipeline architecture acceptance through migrated script entrypoints."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "pipeline-architecture-acceptance.v00.01"
REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SUMMARY = REPO_ROOT / "data" / "pipeline-script-architecture" / "acceptance-summary.json"
ACCEPTANCE_INGEST_REPORT = (
    REPO_ROOT / "data" / "pipeline-script-architecture" / "acceptance-ingest-report.md"
)


@dataclass(frozen=True)
class AcceptancePhase:
    """One migrated wrapper phase in dependency order."""

    name: str
    command: tuple[str, ...]
    artifacts: tuple[Path, ...]
    count_artifact: Path | None = None
    count_fields: tuple[str, ...] = ()
    required: bool = True
    description: str = ""


@dataclass(frozen=True)
class PhaseResult:
    """Serializable phase execution result."""

    name: str
    status: str
    command: tuple[str, ...]
    duration_ms: int
    exit_code: int | None
    artifacts: tuple[dict[str, Any], ...]
    counts: dict[str, Any]
    first_failure: dict[str, Any] | None = None
    stdout_tail: str = ""
    stderr_tail: str = ""


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY)
    return parser.parse_args(argv)


def default_phases() -> tuple[AcceptancePhase, ...]:
    """Return M122 acceptance phases in dependency order."""

    return (
        AcceptancePhase(
            name="catalog_ingest_wrapper",
            description="M061 canonical catalog ingest compatibility wrapper, offline/no-index.",
            command=(
                "uv",
                "run",
                "python",
                "scripts/ingest_to_canonical_catalog.py",
                "--no-network",
                "--no-index",
                "--report-path",
                _display_path(ACCEPTANCE_INGEST_REPORT),
            ),
            artifacts=(ACCEPTANCE_INGEST_REPORT,),
        ),
        AcceptancePhase(
            name="parser_replay_wrapper",
            description="R024/M121 218-document parser replay compatibility wrapper.",
            command=("uv", "run", "python", "scripts/replay_r024_218_document_parser_chunking.py"),
            artifacts=(
                REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "parser-chunking" / "summary.json",
                REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "parser-chunking" / "events.jsonl",
            ),
            count_artifact=REPO_ROOT
            / "data"
            / "r024-218-document-corpus-v1"
            / "parser-chunking"
            / "summary.json",
            count_fields=("total", "ok", "skipped", "errors", "chunk_count_total"),
        ),
        AcceptancePhase(
            name="networkx_probe_wrapper",
            description="R024/M121 218-document NetworkX graph probe compatibility wrapper.",
            command=("uv", "run", "python", "scripts/build_r024_218_document_networkx_probe.py"),
            artifacts=(
                REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "networkx-probe" / "summary.json",
                REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "networkx-probe" / "memory-profile.json",
                REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "networkx-probe" / "events.jsonl",
                REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "networkx-probe" / "probe.graphml",
            ),
            count_artifact=REPO_ROOT
            / "data"
            / "r024-218-document-corpus-v1"
            / "networkx-probe"
            / "summary.json",
            count_fields=(
                "total_catalog_records_seen",
                "corpus_size",
                "skipped_metadata_only",
                "chunk_count_total",
                "n_nodes",
                "n_edges",
                "citation_relations_count",
            ),
        ),
        AcceptancePhase(
            name="coverage_report_wrapper",
            description="R024/M121 coverage report compatibility wrapper.",
            command=("uv", "run", "python", "scripts/build_r024_coverage_report.py"),
            artifacts=(
                REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "R024-COVERAGE.md",
                REPO_ROOT / "data" / "r024-218-document-corpus-v1" / "coverage-summary.json",
            ),
            count_artifact=REPO_ROOT
            / "data"
            / "r024-218-document-corpus-v1"
            / "coverage-summary.json",
            count_fields=(
                "catalog_records",
                "source_backed_records",
                "metadata_only_records",
                "parser_errors",
                "graph_nodes",
                "graph_edges",
                "citation_relations",
            ),
        ),
    )


def run_acceptance(
    *,
    phases: tuple[AcceptancePhase, ...],
    summary_path: Path,
    cwd: Path = REPO_ROOT,
) -> dict[str, Any]:
    """Run phases fail-fast and write acceptance summary."""

    started_at = datetime.now(UTC).isoformat()
    phase_results: list[PhaseResult] = []
    succeeded = True
    first_failure: dict[str, Any] | None = None

    for phase in phases:
        result = run_phase(phase, cwd=cwd)
        phase_results.append(result)
        if result.status != "pass":
            succeeded = False
            first_failure = result.first_failure
            break

    summary = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": datetime.now(UTC).isoformat(),
        "started_at": started_at,
        "succeeded": succeeded,
        "phase_count": len(phase_results),
        "phases": [_phase_result_payload(result) for result in phase_results],
        "first_failure": first_failure,
    }
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    return summary


def run_phase(phase: AcceptancePhase, *, cwd: Path = REPO_ROOT) -> PhaseResult:
    """Run one phase and return compact observable result."""

    start = time.perf_counter()
    completed = subprocess.run(
        phase.command,
        cwd=cwd,
        check=False,
        text=True,
        capture_output=True,
    )
    duration_ms = int((time.perf_counter() - start) * 1000)
    artifacts = tuple(_artifact_status(path) for path in phase.artifacts)
    counts = _load_counts(phase.count_artifact, phase.count_fields)

    failure = _first_failure(
        phase=phase,
        exit_code=completed.returncode,
        artifacts=artifacts,
        stderr_tail=_tail(completed.stderr),
    )
    return PhaseResult(
        name=phase.name,
        status="pass" if failure is None else "fail",
        command=phase.command,
        duration_ms=duration_ms,
        exit_code=completed.returncode,
        artifacts=artifacts,
        counts=counts,
        first_failure=failure,
        stdout_tail=_tail(completed.stdout),
        stderr_tail=_tail(completed.stderr),
    )


def _first_failure(
    *,
    phase: AcceptancePhase,
    exit_code: int,
    artifacts: tuple[dict[str, Any], ...],
    stderr_tail: str,
) -> dict[str, Any] | None:
    if exit_code != 0:
        return {
            "phase": phase.name,
            "code": "nonzero_exit",
            "exit_code": exit_code,
            "stderr_tail": stderr_tail,
        }
    missing = [artifact["path"] for artifact in artifacts if not artifact["exists"]]
    if missing:
        return {
            "phase": phase.name,
            "code": "missing_artifact",
            "missing_artifacts": missing,
        }
    return None


def _load_counts(path: Path | None, fields: tuple[str, ...]) -> dict[str, Any]:
    if path is None or not fields or not path.exists():
        return {}
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        return {}
    return {field: data.get(field) for field in fields if field in data}


def _artifact_status(path: Path) -> dict[str, Any]:
    return {
        "path": _display_path(path),
        "exists": path.exists(),
        "size_bytes": path.stat().st_size if path.exists() else 0,
    }


def _phase_result_payload(result: PhaseResult) -> dict[str, Any]:
    return {
        "name": result.name,
        "status": result.status,
        "command": list(result.command),
        "duration_ms": result.duration_ms,
        "exit_code": result.exit_code,
        "artifacts": list(result.artifacts),
        "counts": result.counts,
        "first_failure": result.first_failure,
        "stdout_tail": result.stdout_tail,
        "stderr_tail": result.stderr_tail,
    }


def _tail(value: str, *, max_chars: int = 1200) -> str:
    return value[-max_chars:] if len(value) > max_chars else value


def _display_path(path: Path) -> str:
    try:
        return path.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    summary = run_acceptance(phases=default_phases(), summary_path=args.summary)
    print(f"summary={args.summary}")
    print(f"succeeded={str(summary['succeeded']).lower()}")
    if summary["first_failure"]:
        print(f"first_failure={summary['first_failure']}")
    return 0 if summary["succeeded"] else 1


if __name__ == "__main__":
    sys.exit(main())
