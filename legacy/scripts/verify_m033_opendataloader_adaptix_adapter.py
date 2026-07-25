#!/usr/bin/env python3
"""Validate M033 OpenDataLoader Adaptix adapter probe artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

SAFETY_FALSE_KEYS = [
    "graph_import_allowed",
    "ladybugdb_written",
    "production_import_attempted",
    "trusted_kg_import_allowed",
    "import_eligible",
]


def read_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise TypeError(f"Expected JSON object at {path}")
    return data


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        row = json.loads(line)
        if not isinstance(row, dict):
            raise TypeError(f"Expected JSON object on {path}:{line_no}")
        rows.append(row)
    return rows


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def check_false_flags(payload: dict[str, Any], where: str, failures: list[dict[str, Any]]) -> None:
    flags = payload.get("safety_flags")
    if not isinstance(flags, dict):
        failures.append({"code": "missing_safety_flags", "where": where})
        return
    for key in SAFETY_FALSE_KEYS:
        if flags.get(key) is not False:
            failures.append(
                {"code": "unsafe_flag", "where": where, "flag": key, "value": flags.get(key)}
            )


def verify(probe_root: Path, adapter_dir: Path) -> int:
    summary_path = adapter_dir / "adaptix-adapter-summary.json"
    diagnostics_path = adapter_dir / "adaptix-adapter-diagnostics.jsonl"
    report_path = adapter_dir / "adaptix-adapter-report.md"
    failures: list[dict[str, Any]] = []
    for path in [summary_path, diagnostics_path, report_path]:
        if not path.exists() or path.stat().st_size == 0:
            failures.append({"code": "missing_or_empty_artifact", "path": str(path)})
    if failures:
        closeout = {
            "schema": "m033.opendataloader_adaptix_adapter.closeout.v1",
            "status": "failed",
            "failure_count": len(failures),
            "failures": failures,
        }
        write_json(adapter_dir / "adaptix-adapter-closeout-summary.json", closeout)
        return 1

    summary = read_json(summary_path)
    diagnostics = read_jsonl(diagnostics_path)
    report = report_path.read_text(encoding="utf-8")
    expected_paths = sorted(probe_root.glob("per-paper/*/hybrid/original.json"))
    expected_keys = {path.parents[1].name for path in expected_paths}

    if summary.get("schema") != "m033.opendataloader_adaptix_adapter.summary.v1":
        failures.append({"code": "wrong_schema", "path": str(summary_path)})
    if summary.get("status") != "adaptix-adapter-candidate":
        failures.append({"code": "wrong_status", "status": summary.get("status")})
    if summary.get("paper_count") != len(expected_keys):
        failures.append(
            {
                "code": "paper_count_mismatch",
                "expected": len(expected_keys),
                "actual": summary.get("paper_count"),
            }
        )
    if summary.get("error_count") != 0:
        failures.append(
            {"code": "adapter_errors_present", "error_count": summary.get("error_count")}
        )
    check_false_flags(summary, "summary", failures)

    results = summary.get("results")
    if not isinstance(results, list):
        failures.append({"code": "results_not_list"})
        results = []
    result_keys = {result.get("article_key") for result in results if isinstance(result, dict)}
    if result_keys != expected_keys:
        failures.append(
            {
                "code": "article_keys_mismatch",
                "expected": sorted(expected_keys),
                "actual": sorted(str(key) for key in result_keys),
            }
        )
    for result in results:
        if not isinstance(result, dict):
            failures.append({"code": "result_not_object"})
            continue
        article_key = str(result.get("article_key"))
        if result.get("status") != "mapped_candidate_only":
            failures.append(
                {
                    "code": "unexpected_result_status",
                    "article_key": article_key,
                    "status": result.get("status"),
                }
            )
        check_false_flags(result, f"result:{article_key}", failures)
        candidate = result.get("candidate_summary")
        if not isinstance(candidate, dict):
            failures.append({"code": "missing_candidate_summary", "article_key": article_key})
            continue
        source_ref = candidate.get("source_ref_candidate", {})
        page_index = candidate.get("page_index_candidate", {})
        if source_ref.get("candidate_only") is not True:
            failures.append({"code": "source_ref_not_candidate_only", "article_key": article_key})
        if (
            not isinstance(page_index.get("top_level_element_count"), int)
            or page_index.get("top_level_element_count", 0) <= 0
        ):
            failures.append({"code": "missing_top_level_elements", "article_key": article_key})

    if any(row.get("severity") == "error" for row in diagnostics):
        failures.append({"code": "error_diagnostic_present"})
    for row in diagnostics:
        check_false_flags(row, f"diagnostic:{row.get('article_key')}", failures)
    for forbidden in ["graph-ready", "import eligible", "ladybugdb write allowed"]:
        if forbidden in report.lower():
            failures.append({"code": "forbidden_positive_claim_in_report", "phrase": forbidden})

    status = "passed" if not failures else "failed"
    closeout = {
        "schema": "m033.opendataloader_adaptix_adapter.closeout.v1",
        "status": status,
        "failure_count": len(failures),
        "paper_count": summary.get("paper_count"),
        "diagnostic_count": len(diagnostics),
        "verdict": summary.get("status"),
        "failures": failures,
        "safety_flags": dict.fromkeys(SAFETY_FALSE_KEYS, False),
    }
    write_json(adapter_dir / "adaptix-adapter-closeout-summary.json", closeout)
    lines = [
        "# M033 OpenDataLoader Adaptix Adapter Closeout",
        "",
        f"Status: `{status}`",
        f"Failure count: `{len(failures)}`",
        f"Verdict: `{summary.get('status')}`",
        "",
        "Safety flags remain false for graph/import/LadybugDB eligibility.",
    ]
    if failures:
        lines += ["", "## Failures", ""]
        for failure in failures:
            lines.append(f"- `{failure['code']}` {failure}")
    (adapter_dir / "adaptix-adapter-closeout-report.md").write_text(
        "\n".join(lines) + "\n", encoding="utf-8"
    )
    sys.stdout.write(
        json.dumps(
            {
                "status": status,
                "failure_count": len(failures),
                "paper_count": summary.get("paper_count"),
            },
            indent=2,
        )
        + "\n"
    )
    return 0 if status == "passed" else 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe-root", type=Path, required=True)
    parser.add_argument("--adapter-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    return verify(args.probe_root, args.adapter_dir)


if __name__ == "__main__":
    raise SystemExit(main())
