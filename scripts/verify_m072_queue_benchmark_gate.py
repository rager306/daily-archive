#!/usr/bin/env python3
"""Verify M072 benchmark metrics can flow through UniversalKBQueue metadata.

This script is deterministic and local-only. It does not call MiniMax, DSPy,
FalkorDB, or any external service, and it does not authorize graph writes or
fact promotion.
"""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path
from typing import Any

from research_graph.workflows.universal_kb.queue import UniversalKBQueue


class FixedClock:
    def __call__(self) -> str:
        return "2026-06-16T00:00:00Z"


def _diagnostics_from_results(results: dict[str, Any]) -> dict[str, Any]:
    diagnostics: dict[str, Any] = {}
    for split in ("train", "validation"):
        metrics = results[split]
        diagnostics[f"{split}_entity_f1"] = metrics["entity_f1"]
        diagnostics[f"{split}_relation_f1"] = metrics["relation_f1"]
        diagnostics[f"{split}_evidence_path_validity"] = metrics["evidence_path_validity"]
        diagnostics[f"{split}_schema_validity"] = metrics["schema_validity"]
        diagnostics[f"{split}_json_validity"] = metrics["json_validity"]
    return diagnostics


def verify(results_path: Path, output_path: Path) -> dict[str, Any]:
    results = json.loads(results_path.read_text())
    diagnostics = _diagnostics_from_results(results)
    total_retry_count = sum(int(results[split]["total_retry_count"]) for split in ("train", "validation"))
    mean_cost = sum(float(results[split]["mean_cost_estimate"]) for split in ("train", "validation")) / 2
    mean_latency = int(sum(float(results[split]["mean_latency_ms"]) for split in ("train", "validation")) / 2)

    with tempfile.TemporaryDirectory() as tmpdir:
        queue = UniversalKBQueue(Path(tmpdir) / "m072-queue.sqlite", clock=FixedClock()).initialize()
        queue.enqueue(
            job_id="job-m072-reviewed-benchmark",
            stage="benchmark",
            input_refs=("artifact:m072-reviewed-fixtures",),
            input_hash="sha256:m072-reviewed-fixtures",
            tool_version="tool:m072_benchmark",
            contract_version="contract:m072_v1",
            payload_metadata={
                "schema_version": "schema:m072_v1",
                "metric_bundle_id": "metric_bundle:m072_v1",
                "extractor_version": "extractor:fixture_baseline_v1",
                "source_artifact_refs": ["artifact:m072-reviewed-fixtures"],
            },
        )
        updated = queue.update_payload_diagnostics(
            "job-m072-reviewed-benchmark",
            diagnostics=diagnostics,
            cost_estimate=mean_cost,
            latency_ms=mean_latency,
            retry_count=total_retry_count,
            evidence_path_refs=("evidence:m072:reviewed_benchmark",),
        )

    payload = updated["payload_metadata"]
    if payload["write_eligibility"] is not False:
        raise RuntimeError("write_eligibility changed from false")
    if payload["promotion_eligibility"] is not False:
        raise RuntimeError("promotion_eligibility changed from false")
    if payload["diagnostics"].get("validation_relation_f1") != results["validation"]["relation_f1"]:
        raise RuntimeError("validation relation metric was not persisted")

    report = {
        "status": "PASS",
        "job_id": updated["job_id"],
        "write_eligibility": payload["write_eligibility"],
        "promotion_eligibility": payload["promotion_eligibility"],
        "diagnostics": payload["diagnostics"],
        "cost_estimate": payload["cost_estimate"],
        "latency_ms": payload["latency_ms"],
        "retry_count": payload["retry_count"],
        "evidence_path_refs": payload["evidence_path_refs"],
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = verify(args.results, args.output)
    print(json.dumps({"status": report["status"], "job_id": report["job_id"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
