#!/usr/bin/env python3
"""Run local diagnostic-only quality checks for touched Python modules.

This script intentionally produces maintainability diagnostics without acting as
an enforcement gate. A successful process exit means the diagnostic runner
completed and wrote artifacts; it does not mean risk scores are acceptable.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import Any

from arxiv_archive.quality import build_maintainability_report, write_maintainability_report

DEFAULT_BASE_REF = "HEAD"
DEFAULT_OUTPUT_DIR = Path("artifacts/quality")
DEFAULT_DIAGNOSTIC_SCOPE = (
    Path("src/arxiv_archive/quality/riskratchet_adapter.py"),
    Path("src/arxiv_archive/quality/maintainability_report.py"),
    Path("src/arxiv_archive/quality/baselines.py"),
)
JSON_REPORT_NAME = "maintainability-diagnostic.json"
HUMAN_REPORT_NAME = "maintainability-diagnostic.md"


def gather_touched_python_modules(*, base_ref: str = DEFAULT_BASE_REF, fallback: Sequence[Path] = DEFAULT_DIAGNOSTIC_SCOPE) -> tuple[Path, ...]:
    """Return touched source/script Python files, falling back to the quality scope.

    Git discovery is best-effort because this is a local diagnostic helper. If git
    is unavailable, the checkout is shallow, or no source Python files are
    currently touched, the runner still emits a reproducible report over the
    diagnostic quality modules introduced by this slice.
    """
    try:
        result = subprocess.run(
            ["git", "diff", "--name-only", "--diff-filter=ACMRT", base_ref, "--", "*.py"],
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired):
        return tuple(fallback)

    if result.returncode != 0:
        return tuple(fallback)

    touched = tuple(
        Path(line.strip())
        for line in result.stdout.splitlines()
        if _is_quality_scan_candidate(line.strip())
    )
    return touched or tuple(fallback)


def run_quality_gate(
    *,
    paths: Sequence[str | Path] | None = None,
    output_dir: str | Path = DEFAULT_OUTPUT_DIR,
    baseline_path: str | Path | None = None,
    base_ref: str = DEFAULT_BASE_REF,
) -> dict[str, Any]:
    """Run the diagnostic-only maintainability scan and write JSON/Markdown artifacts."""
    scan_paths = tuple(Path(path) for path in paths) if paths else gather_touched_python_modules(base_ref=base_ref)
    report = build_maintainability_report(paths=scan_paths, baseline_path=baseline_path)
    report["quality_gate"] = {
        "name": "local-maintainability-diagnostic",
        "diagnostic_only": True,
        "blocking": False,
        "pass_fail_affected": False,
        "touched_module_count": len(scan_paths),
        "touched_modules": [str(path) for path in scan_paths],
        "human_report": str(Path(output_dir) / HUMAN_REPORT_NAME),
        "json_report": str(Path(output_dir) / JSON_REPORT_NAME),
    }

    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)
    json_path = write_maintainability_report(report, output_path / JSON_REPORT_NAME)
    markdown_path = write_human_report(report, output_path / HUMAN_REPORT_NAME)
    report["output_paths"] = {"json": str(json_path), "human": str(markdown_path)}

    # Rewrite JSON with the final output_paths envelope for consumers that only
    # read the artifact from disk.
    write_maintainability_report(report, json_path)
    return report


def write_human_report(report: dict[str, Any], output_path: str | Path) -> Path:
    """Write a compact human-readable diagnostic report."""
    path = Path(output_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(_render_human_report(report), encoding="utf-8")
    return path


def _render_human_report(report: dict[str, Any]) -> str:
    summary = report.get("summary", {}) if isinstance(report.get("summary"), dict) else {}
    delta = report.get("baseline_delta", {}) if isinstance(report.get("baseline_delta"), dict) else {}
    severity = summary.get("by_severity", {}) if isinstance(summary.get("by_severity"), dict) else {}
    quality_gate = report.get("quality_gate", {}) if isinstance(report.get("quality_gate"), dict) else {}
    modules = quality_gate.get("touched_modules", []) if isinstance(quality_gate.get("touched_modules"), list) else []

    lines = [
        "# Local Maintainability Diagnostic",
        "",
        "**Diagnostic-only:** yes. This report is non-blocking and does not change smoke/test pass-fail status.",
        "",
        f"- Status: `{report.get('status', 'unknown')}`",
        f"- Tool status: `{report.get('tool_status', 'unknown')}`",
        f"- Blocking: `{str(report.get('blocking', False)).lower()}`",
        f"- Pass/fail affected: `{str(report.get('pass_fail_affected', False)).lower()}`",
        f"- Total functions: `{summary.get('total_functions', 0)}`",
        f"- Max score: `{summary.get('max_score', 0.0)}`",
        f"- Average score: `{summary.get('average_score', 0.0)}`",
        f"- Severity bands: `{json.dumps(severity, sort_keys=True)}`",
        f"- Baseline present: `{str(delta.get('baseline_present', False)).lower()}`",
        f"- Max score delta: `{delta.get('max_score_delta')}`",
        f"- Average score delta: `{delta.get('average_score_delta')}`",
        "",
        "## Touched Modules",
    ]
    lines.extend(f"- `{module}`" for module in modules)
    if not modules:
        lines.append("- none discovered")
    if report.get("tool_error"):
        lines.extend(["", "## Diagnostic Tool Error", "", str(report["tool_error"])])
    return "\n".join(lines) + "\n"


def _is_quality_scan_candidate(path: str) -> bool:
    return path.endswith(".py") and (path.startswith("src/arxiv_archive/") or path.startswith("scripts/"))


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run the local diagnostic-only maintainability quality gate.")
    parser.add_argument("paths", nargs="*", type=Path, help="Explicit Python files/directories to scan. Defaults to touched modules.")
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR, help="Directory for diagnostic artifacts.")
    parser.add_argument("--baseline", type=Path, default=None, help="Optional JSON baseline for non-blocking deltas.")
    parser.add_argument("--base-ref", default=DEFAULT_BASE_REF, help="Git ref used for touched-module discovery when paths are omitted.")
    parser.add_argument("--json", action="store_true", help="Print the report envelope as JSON.")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    report = run_quality_gate(
        paths=args.paths or None,
        output_dir=args.output_dir,
        baseline_path=args.baseline,
        base_ref=args.base_ref,
    )
    if args.json:
        sys.stdout.write(json.dumps(report, indent=2, sort_keys=True) + "\n")
    else:
        output_paths = report.get("output_paths", {})
        summary = report.get("summary", {})
        sys.stdout.write(
            " | ".join(
                [
                    f"status: {report.get('status')}",
                    "diagnostic only: true",
                    "blocking: false",
                    f"functions: {summary.get('total_functions', 0)}",
                    f"max score: {summary.get('max_score', 0.0)}",
                    f"human report: {output_paths.get('human')}",
                    f"json report: {output_paths.get('json')}",
                ]
            )
            + "\n"
        )
    return 0


if __name__ == "__main__":  # pragma: no cover - exercised by subprocess users
    raise SystemExit(main())
