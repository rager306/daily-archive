#!/usr/bin/env python3
"""Inventory pytest files against the project test-layer taxonomy.

This is an audit tool, not an enforcement gate. It classifies tests by import
signals so M128 can align the suite with hexagonal/onion boundaries without
rewriting legacy coverage blindly.
"""

from __future__ import annotations

import argparse
import ast
import json
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "daily-archive-test-architecture-inventory.v1"
DEFAULT_TESTS_DIR = Path("tests")
DEFAULT_OUTPUT_DIR = Path("data/test-architecture-alignment")

RECENT_PILOT_PREFIXES = (
    "tests/test_catalog_ingest_",
    "tests/test_corpus_coverage_",
    "tests/test_graph_probe_",
    "tests/test_m122_",
    "tests/test_networkx_graph_probe_adapter.py",
    "tests/test_parser_replay_",
    "tests/test_pipeline_architecture_acceptance.py",
    "tests/test_pipeline_script_",
    "tests/test_riskratchet_gate.py",
)

BUCKET_ORDER = (
    "domain",
    "application",
    "infrastructure",
    "script-wrapper",
    "acceptance",
    "legacy-mixed",
    "unknown",
)


@dataclass(frozen=True)
class TestFileAnalysis:
    path: Path
    imports: tuple[str, ...]
    signals: dict[str, bool]
    bucket: str
    reasons: tuple[str, ...]

    def as_dict(self) -> dict[str, Any]:
        return {
            "path": self.path.as_posix(),
            "bucket": self.bucket,
            "signals": self.signals,
            "imports": list(self.imports),
            "reasons": list(self.reasons),
        }


def analyze_test_file(path: Path) -> TestFileAnalysis:
    text = path.read_text(encoding="utf-8", errors="replace")
    imports = _collect_imports(path, text)
    signal_values = {
        "imports_domain": any(name.startswith("research_graph.domain") for name in imports),
        "imports_application": any(
            name.startswith("research_graph.application") for name in imports
        ),
        "imports_infrastructure": any(
            name.startswith("research_graph.infrastructure") for name in imports
        ),
        "imports_workflows": any(name.startswith("research_graph.workflows") for name in imports),
        "imports_cli": any(name.startswith("research_graph.cli") for name in imports),
        "imports_pipeline_legacy": any(name.startswith("research_graph.pipeline") for name in imports),
        "imports_scripts_normal": any(
            name == "scripts" or name.startswith("scripts.") for name in imports
        ),
        "dynamic_script_import": "spec_from_file_location" in text,
        "subprocess_script_invocation": "subprocess" in text and "scripts/" in text,
        "acceptance_name": "acceptance" in path.name or "acceptance" in text,
    }
    bucket, reasons = classify(path, signal_values)
    return TestFileAnalysis(
        path=path,
        imports=tuple(sorted(imports)),
        signals=signal_values,
        bucket=bucket,
        reasons=tuple(reasons),
    )


def _collect_imports(path: Path, text: str) -> set[str]:
    try:
        tree = ast.parse(text, filename=str(path))
    except SyntaxError:
        return set()

    imports: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                imports.add(alias.name)
        elif isinstance(node, ast.ImportFrom):
            if node.module:
                imports.add(node.module)
    return imports


def classify(path: Path, signals: dict[str, bool]) -> tuple[str, list[str]]:
    reasons: list[str] = []
    if signals["acceptance_name"]:
        reasons.append("acceptance marker in file name or content")
        return "acceptance", reasons
    if signals["dynamic_script_import"]:
        reasons.append("dynamic script import via spec_from_file_location")
        return "legacy-mixed", reasons
    if signals["imports_scripts_normal"] or signals["subprocess_script_invocation"]:
        reasons.append("script wrapper or subprocess invocation")
        return "script-wrapper", reasons
    if signals["imports_infrastructure"]:
        reasons.append("imports infrastructure adapter layer")
        return "infrastructure", reasons
    if signals["imports_application"]:
        reasons.append("imports application layer without infrastructure")
        return "application", reasons
    if signals["imports_domain"]:
        reasons.append("imports domain layer only")
        return "domain", reasons
    if signals["imports_workflows"] or signals["imports_cli"] or signals["imports_pipeline_legacy"]:
        reasons.append("imports workflow, CLI, or legacy pipeline surface")
        return "legacy-mixed", reasons
    reasons.append("no recognized project-layer import signal")
    return "unknown", reasons


def build_inventory(tests_dir: Path = DEFAULT_TESTS_DIR) -> dict[str, Any]:
    analyses = [analyze_test_file(path) for path in sorted(tests_dir.glob("test_*.py"))]
    bucket_counts = Counter(analysis.bucket for analysis in analyses)
    signal_counts: Counter[str] = Counter()
    for analysis in analyses:
        for signal, value in analysis.signals.items():
            if value:
                signal_counts[signal] += 1

    files = [analysis.as_dict() for analysis in analyses]
    pilot_candidates = choose_pilot_candidates(analyses)
    return {
        "schema_version": SCHEMA_VERSION,
        "summary": {
            "total_test_files": len(analyses),
            "buckets": {bucket: bucket_counts.get(bucket, 0) for bucket in BUCKET_ORDER},
            "signals": dict(sorted(signal_counts.items())),
        },
        "files": files,
        "pilot_candidates": pilot_candidates,
    }


def choose_pilot_candidates(analyses: list[TestFileAnalysis]) -> list[dict[str, str]]:
    selected: list[dict[str, str]] = []
    by_path = {analysis.path.as_posix(): analysis for analysis in analyses}
    for prefix in RECENT_PILOT_PREFIXES:
        for path_text in sorted(path for path in by_path if path.startswith(prefix)):
            analysis = by_path[path_text]
            if analysis.bucket == "legacy-mixed":
                continue
            selected.append(
                {
                    "path": path_text,
                    "current_bucket": analysis.bucket,
                    "suggested_layer": suggested_layer_for(analysis),
                    "rationale": "; ".join(analysis.reasons),
                }
            )
    # Keep the pilot bounded and stable.
    deduped: list[dict[str, str]] = []
    seen: set[str] = set()
    for candidate in selected:
        if candidate["path"] in seen:
            continue
        seen.add(candidate["path"])
        deduped.append(candidate)
        if len(deduped) >= 16:
            break
    return deduped


def suggested_layer_for(analysis: TestFileAnalysis) -> str:
    if analysis.bucket in {"domain", "application", "infrastructure", "acceptance"}:
        return analysis.bucket
    if analysis.bucket == "script-wrapper":
        return "script-wrapper"
    return "needs-review"


def write_outputs(inventory: dict[str, Any], output_dir: Path) -> tuple[Path, Path, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    json_path = output_dir / "test-architecture-inventory.json"
    markdown_path = output_dir / "test-architecture-inventory.md"
    pilot_path = output_dir / "pilot-candidates.json"
    json_path.write_text(json.dumps(inventory, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    markdown_path.write_text(render_markdown(inventory), encoding="utf-8")
    pilot_path.write_text(
        json.dumps(inventory["pilot_candidates"], indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return json_path, markdown_path, pilot_path


def render_markdown(inventory: dict[str, Any]) -> str:
    summary = inventory["summary"]
    lines = [
        "# Test Architecture Inventory",
        "",
        f"Schema: `{inventory['schema_version']}`",
        "",
        "## Summary",
        "",
        f"- Total test files: `{summary['total_test_files']}`",
        "",
        "### Buckets",
        "",
        "| Bucket | Count |",
        "|---|---:|",
    ]
    for bucket, count in summary["buckets"].items():
        lines.append(f"| `{bucket}` | {count} |")
    lines.extend(["", "### Import and execution signals", "", "| Signal | Count |", "|---|---:|"])
    for signal, count in summary["signals"].items():
        lines.append(f"| `{signal}` | {count} |")

    lines.extend(["", "## Representative files by bucket", ""])
    by_bucket: dict[str, list[dict[str, Any]]] = {bucket: [] for bucket in BUCKET_ORDER}
    for item in inventory["files"]:
        by_bucket.setdefault(item["bucket"], []).append(item)
    for bucket in BUCKET_ORDER:
        lines.extend([f"### {bucket}", ""])
        examples = by_bucket.get(bucket, [])[:10]
        if not examples:
            lines.append("- none")
        else:
            for item in examples:
                reason = "; ".join(item["reasons"])
                lines.append(f"- `{item['path']}` — {reason}")
        lines.append("")

    lines.extend(["## Pilot candidates", "", "| Path | Current bucket | Suggested layer |", "|---|---|---|"])
    for candidate in inventory["pilot_candidates"]:
        lines.append(
            f"| `{candidate['path']}` | `{candidate['current_bucket']}` | `{candidate['suggested_layer']}` |"
        )
    return "\n".join(lines) + "\n"


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Audit pytest files by architecture layer.")
    parser.add_argument("--tests-dir", type=Path, default=DEFAULT_TESTS_DIR)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--json", action="store_true", help="Print the inventory JSON to stdout.")
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    inventory = build_inventory(args.tests_dir)
    json_path, markdown_path, pilot_path = write_outputs(inventory, args.output_dir)
    if args.json:
        print(json.dumps(inventory, indent=2, sort_keys=True))
    else:
        summary = inventory["summary"]
        print(
            " | ".join(
                [
                    "test architecture inventory",
                    f"files: {summary['total_test_files']}",
                    f"json: {json_path}",
                    f"markdown: {markdown_path}",
                    f"pilot: {pilot_path}",
                ]
            )
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
