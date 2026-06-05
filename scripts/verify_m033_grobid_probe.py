#!/usr/bin/env python3
"""Validate M033 S02 GROBID bounded probe artifacts.

This verifier is intentionally fail-closed: GROBID output can only be candidate
parser evidence. Any permissive graph/import/write safety flag is a failure.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

FALSE_SAFETY_KEYS = (
    "graph_import_allowed",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
)
OPTIONAL_FALSE_SAFETY_KEYS = (
    "trusted_kg_import_allowed",
    "graph_write_attempted",
    "production_persistence_attempted",
)


def load_json(path: Path, failures: list[dict[str, Any]]) -> dict[str, Any]:
    if not path.exists() or path.stat().st_size == 0:
        failures.append({"code": "missing_json", "path": str(path)})
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        failures.append({"code": "invalid_json", "path": str(path), "error": str(exc)})
        return {}


def require_false_flags(
    owner: str,
    flags: dict[str, Any] | None,
    failures: list[dict[str, Any]],
) -> None:
    if not isinstance(flags, dict):
        failures.append({"code": "missing_safety_flags", "owner": owner})
        return
    for key in FALSE_SAFETY_KEYS:
        if flags.get(key) is not False:
            failures.append(
                {"code": "unsafe_flag", "owner": owner, "flag": key, "value": flags.get(key)}
            )
    for key in OPTIONAL_FALSE_SAFETY_KEYS:
        if key in flags and flags.get(key) is not False:
            failures.append(
                {"code": "unsafe_optional_flag", "owner": owner, "flag": key, "value": flags.get(key)}
            )


def validate_runtime(probe_dir: Path, failures: list[dict[str, Any]]) -> None:
    readiness = load_json(probe_dir / "grobid-runtime-readiness.json", failures)
    if not readiness:
        return
    require_false_flags("runtime_readiness", readiness.get("safety_flags"), failures)
    if readiness.get("selected_image") != "grobid/grobid:0.9.0-crf":
        failures.append({"code": "unexpected_image", "value": readiness.get("selected_image")})
    native = readiness.get("native_build_requirement", {})
    if native.get("jdk_required") != "OpenJDK 21+":
        failures.append({"code": "missing_jdk21_requirement", "value": native.get("jdk_required")})
    docker = readiness.get("docker", {})
    if docker.get("daemon_available") is not True:
        failures.append({"code": "docker_daemon_not_recorded_ready"})
    if docker.get("image_present_after_pull") is not True:
        failures.append({"code": "docker_image_not_verified_after_pull"})


def validate_run(probe_dir: Path, failures: list[dict[str, Any]]) -> dict[str, Any]:
    summary = load_json(probe_dir / "grobid-run-summary.json", failures)
    if not summary:
        return {}
    require_false_flags("run_summary", summary.get("safety_flags"), failures)
    status = summary.get("status")
    if status != "tei-probe-complete":
        failures.append({"code": "unexpected_run_status", "status": status})
    if summary.get("paper_count") != 3:
        failures.append({"code": "unexpected_paper_count", "value": summary.get("paper_count")})
    if summary.get("success_count") != 3 or summary.get("failure_count") != 0:
        failures.append(
            {
                "code": "unexpected_success_failure_counts",
                "success_count": summary.get("success_count"),
                "failure_count": summary.get("failure_count"),
            }
        )
    for result in summary.get("results", []):
        paper = result.get("paper_key", "unknown")
        require_false_flags(f"run_result:{paper}", result.get("safety_flags"), failures)
        tei_path = Path(str(result.get("tei_path", "")))
        if result.get("status") != "tei_written":
            failures.append({"code": "tei_not_written", "paper_key": paper})
        if not tei_path.exists() or tei_path.stat().st_size <= 1000:
            failures.append({"code": "tei_missing_or_too_small", "paper_key": paper, "path": str(tei_path)})
        else:
            text = tei_path.read_text(encoding="utf-8", errors="replace")[:2000]
            if "<TEI" not in text and "<tei:TEI" not in text:
                failures.append({"code": "tei_root_not_detected", "paper_key": paper})
            if "teiHeader" not in text:
                failures.append({"code": "tei_header_not_detected", "paper_key": paper})
        diag_path = tei_path.parent / "request-diagnostics.json"
        if not diag_path.exists() or diag_path.stat().st_size == 0:
            failures.append({"code": "missing_request_diagnostics", "paper_key": paper})
    return summary


def validate_mapping(probe_dir: Path, failures: list[dict[str, Any]]) -> None:
    quality = load_json(probe_dir / "grobid-tei-quality-summary.json", failures)
    verdict = load_json(probe_dir / "grobid-probe-verdict.json", failures)
    report_path = probe_dir / "grobid-contract-mapping.md"
    if not report_path.exists() or report_path.stat().st_size == 0:
        failures.append({"code": "missing_mapping_report", "path": str(report_path)})
        report = ""
    else:
        report = report_path.read_text(encoding="utf-8")

    if quality:
        require_false_flags("quality_summary", quality.get("safety_flags"), failures)
        if quality.get("status") != "grobid-tei-candidate-evidence":
            failures.append({"code": "unexpected_quality_status", "status": quality.get("status")})
        if quality.get("paper_count") != 3:
            failures.append({"code": "unexpected_quality_paper_count", "value": quality.get("paper_count")})
        coverage = quality.get("coverage", {})
        for key in (
            "papers_with_title",
            "papers_with_abstract",
            "papers_with_body_divs",
            "papers_with_bibliography",
            "papers_with_coordinates",
        ):
            if coverage.get(key) != 3:
                failures.append({"code": "coverage_gap", "field": key, "value": coverage.get(key)})
        for paper in quality.get("papers", []):
            paper_key = paper.get("paper_key", "unknown")
            require_false_flags(f"quality_paper:{paper_key}", paper.get("safety_flags"), failures)
            if paper.get("body_div_count", 0) <= 0 or paper.get("bibliography_entry_count", 0) <= 0:
                failures.append({"code": "paper_structural_gap", "paper_key": paper_key})

    if verdict:
        require_false_flags("probe_verdict", verdict.get("safety_flags"), failures)
        if verdict.get("verdict") != "grobid-scholarly-sidecar-candidate":
            failures.append({"code": "unexpected_verdict", "value": verdict.get("verdict")})
        if verdict.get("candidate_only") is not True:
            failures.append({"code": "verdict_not_candidate_only"})

    for needle in (
        "grobid-scholarly-sidecar-candidate",
        "graph_import_allowed=false",
        "ladybugdb_written=false",
        "production_import_attempted=false",
        "import_eligible=false",
        "not graph-ready",
    ):
        if needle not in report:
            failures.append({"code": "missing_report_boundary", "needle": needle})


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def write_report(path: Path, closeout: dict[str, Any]) -> None:
    lines = [
        "# M033 S02 GROBID Closeout Report",
        "",
        f"- status: `{closeout['status']}`",
        f"- failure_count: `{len(closeout['failures'])}`",
        f"- verdict: `{closeout['verdict']}`",
        "- candidate_only: `true`",
        "- graph_import_allowed=false",
        "- ladybugdb_written=false",
        "- production_import_attempted=false",
        "- import_eligible=false",
        "",
    ]
    if closeout["failures"]:
        lines.append("## Failures")
        lines.append("")
        for failure in closeout["failures"]:
            lines.append(f"- `{failure['code']}` {failure}")
    else:
        lines.extend(
            [
                "## Result",
                "",
                "GROBID CRF probe artifacts are internally consistent bounded research evidence.",
                "They remain candidate-only and are not graph-ready.",
            ]
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--probe-dir", required=True, type=Path)
    args = parser.parse_args()

    probe_dir = args.probe_dir
    failures: list[dict[str, Any]] = []
    if not probe_dir.exists():
        failures.append({"code": "missing_probe_dir", "path": str(probe_dir)})
    else:
        validate_runtime(probe_dir, failures)
        validate_run(probe_dir, failures)
        validate_mapping(probe_dir, failures)

    status = "passed" if not failures else "failed"
    closeout = {
        "schema_version": "m033.grobid.closeout-summary.v1",
        "created_at_epoch": int(time.time()),
        "status": status,
        "verdict": "grobid-scholarly-sidecar-candidate",
        "candidate_only": True,
        "failures": failures,
        "safety_flags": dict.fromkeys(FALSE_SAFETY_KEYS + OPTIONAL_FALSE_SAFETY_KEYS, False),
    }
    write_json(probe_dir / "grobid-closeout-summary.json", closeout)
    write_report(probe_dir / "grobid-closeout-report.md", closeout)
    sys.stdout.write(
        json.dumps(
            {
                "status": status,
                "failure_count": len(failures),
                "verdict": closeout["verdict"],
            },
            indent=2,
        )
        + "\n"
    )
    return 0 if status == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
