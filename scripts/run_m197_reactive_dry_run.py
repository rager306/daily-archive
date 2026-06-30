#!/usr/bin/env python3
"""Run the M197 reactive no-write dry-run pilot.

The command emits metadata-only JSONL events. It does not read source payloads,
contact graph backends, run schema migrations, or mark imports eligible.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
for path in (ROOT, SRC):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from research_graph.workflows.universal_kb.reactive_runner import (  # noqa: E402
    run_reactive_stages_bounded,
)

DEFAULT_EVENTS_PATH = Path("artifacts/m197-reactive-dry-run/events.jsonl")


def _stage_result(*, artifact_ref: str, checksum: str, diagnostic_key: str) -> dict[str, Any]:
    return {
        "artifact_refs": [artifact_ref],
        "child_artifact_refs": [artifact_ref],
        "checksum_sha256": checksum,
        "diagnostics": {diagnostic_key: "metadata_only"},
    }


def _stage_specs() -> list[dict[str, Any]]:
    return [
        {
            "stage_id": "dry_run.schema_gate",
            "phase": "schema_gate",
            "stage": lambda: _stage_result(
                artifact_ref="schema_gate_result.json",
                checksum="1" * 64,
                diagnostic_key="schema_gate",
            ),
            "parent_artifact_refs": ["operator_dry_run_request.json"],
        },
        {
            "stage_id": "dry_run.projection_safety",
            "phase": "projection",
            "stage": lambda: _stage_result(
                artifact_ref="projection_safety_result.json",
                checksum="2" * 64,
                diagnostic_key="projection_safety",
            ),
            "parent_artifact_refs": ["schema_gate_result.json"],
        },
    ]


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--events",
        type=Path,
        default=DEFAULT_EVENTS_PATH,
        help="Path to write JSONL reactive events.",
    )
    parser.add_argument("--job-id", default="m197-reactive-dry-run")
    parser.add_argument("--correlation-id", default="m197-script-dry-run")
    parser.add_argument("--max-concurrency", type=int, default=1)
    return parser


async def _run(args: argparse.Namespace) -> list[dict[str, Any]]:
    return await run_reactive_stages_bounded(
        job_id=args.job_id,
        correlation_id=args.correlation_id,
        max_concurrency=args.max_concurrency,
        stages=_stage_specs(),
    )


def _write_jsonl(path: Path, events: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for event in events:
            handle.write(json.dumps(event, sort_keys=True) + "\n")


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    events = asyncio.run(_run(args))
    _write_jsonl(args.events, events)
    print(f"m197_reactive_events={len(events)}")
    print(f"events_path={args.events}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
