#!/usr/bin/env python3
"""Verify M073 evidence coverage diagnostics fit UniversalKBQueue metadata.

Uses a temporary local SQLite queue only. No network, model calls, graph writes,
production import, or fact promotion are performed.
"""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path
from typing import Any

from arxiv_archive.universal_kb_queue import UniversalKBQueue


class FixedClock:
    def __call__(self) -> str:
        return "2026-06-16T00:00:00Z"


def _diagnostics_from_coverage(coverage: dict[str, Any]) -> dict[str, Any]:
    diagnostics: dict[str, Any] = {}
    for split, metrics in coverage["splits"].items():
        diagnostics[f"{split}_case_count"] = metrics["case_count"]
        diagnostics[f"{split}_canonical_pdf_coverage"] = metrics["canonical_pdf_coverage"]
        diagnostics[f"{split}_parser_manifest_coverage"] = metrics["parser_manifest_coverage"]
        diagnostics[f"{split}_cases_with_missing_diagnostics"] = metrics[
            "cases_with_missing_diagnostics"
        ]
    return diagnostics


def verify(coverage_path: Path, output_path: Path) -> dict[str, Any]:
    coverage = json.loads(coverage_path.read_text())
    diagnostics = _diagnostics_from_coverage(coverage)

    with tempfile.TemporaryDirectory() as tmpdir:
        queue = UniversalKBQueue(Path(tmpdir) / "m073-queue.sqlite", clock=FixedClock()).initialize()
        queue.enqueue(
            job_id="job-m073-parser-evidence-gate",
            stage="benchmark",
            input_refs=("artifact:m073-parser-evidence-fixtures",),
            input_hash="sha256:m073-parser-evidence-fixtures",
            tool_version="tool:m073_evidence_gate",
            contract_version="contract:m073_v1",
            payload_metadata={
                "schema_version": "schema:m073_v1",
                "metric_bundle_id": "metric_bundle:m073_evidence_v1",
                "extractor_version": "extractor:fixture_evidence_refs_v1",
                "source_artifact_refs": ["artifact:m073-parser-evidence-fixtures"],
            },
        )
        updated = queue.update_payload_diagnostics(
            "job-m073-parser-evidence-gate",
            diagnostics=diagnostics,
            cost_estimate=0.0,
            latency_ms=0,
            retry_count=0,
            evidence_path_refs=("evidence:m073:parser_evidence_coverage",),
        )

    payload = updated["payload_metadata"]
    if payload["write_eligibility"] is not False:
        raise RuntimeError("write_eligibility changed from false")
    if payload["promotion_eligibility"] is not False:
        raise RuntimeError("promotion_eligibility changed from false")
    if payload["diagnostics"].get("validation_parser_manifest_coverage") != coverage["splits"][
        "validation"
    ]["parser_manifest_coverage"]:
        raise RuntimeError("validation parser coverage was not persisted")

    report = {
        "status": "PASS",
        "job_id": updated["job_id"],
        "write_eligibility": payload["write_eligibility"],
        "promotion_eligibility": payload["promotion_eligibility"],
        "diagnostics": payload["diagnostics"],
        "evidence_path_refs": payload["evidence_path_refs"],
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--coverage", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = verify(args.coverage, args.output)
    print(json.dumps({"status": report["status"], "job_id": report["job_id"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
