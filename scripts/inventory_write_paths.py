#!/usr/bin/env python3
"""Inventory repo-local Python write paths for architecture review.

This is a deliberately small static scanner. It finds obvious file/database write
operations and applies conservative categories; it is not a data-flow engine.
Unknowns are expected and should be reviewed by humans.
"""

from __future__ import annotations

import argparse
import ast
import json
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

ROOTS = (Path("src/research_graph"), Path("scripts"))
WRITE_ATTRS = {"write_text", "write_bytes"}
OPEN_ATTRS = {"open"}
DB_CONNECT_MODULES = {"sqlite3"}
CALLER_OWNED_TARGET_TOKENS = (
    "filepath",
    "destination",
    "cache_path",
    "markdown_path",
    "json_path",
    "md_path",
    "method_path",
    "review_path",
    "claims_path",
    "memory_profile_path",
    "selection_manifest_path",
    "delta_path",
    "outlier_path",
    "manifests_dir",
    "summary_path",
)
SCHEMA_VERSION = "daily-archive-write-path-inventory.v1"


@dataclass(frozen=True)
class WritePathRecord:
    path: str
    line: int
    operation: str
    target: str
    category: str
    reason: str


def _unparse(node: ast.AST | None) -> str:
    if node is None:
        return "<unknown>"
    try:
        return ast.unparse(node)
    except Exception:  # pragma: no cover - defensive for unusual AST nodes
        return "<unknown>"


def _literal_text(node: ast.AST | None) -> str | None:
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        return node.value
    return None


def _mode_from_call(node: ast.Call) -> str | None:
    if len(node.args) >= 2:
        literal = _literal_text(node.args[1])
        if literal is not None:
            return literal
    for keyword in node.keywords:
        if keyword.arg == "mode":
            literal = _literal_text(keyword.value)
            if literal is not None:
                return literal
    return None


def _call_name(func: ast.AST) -> str:
    if isinstance(func, ast.Attribute):
        return f"{_unparse(func.value)}.{func.attr}"
    return _unparse(func)


def _classify(source_path: Path, operation: str, target: str, mode: str | None) -> tuple[str, str]:
    path_text = f"{source_path} {target}".lower()
    source_text = source_path.as_posix()
    target_text = target.lower()
    if source_text in {
        "scripts/m061_anchor_pilot.py",
        "scripts/m061_full_5_anchors.py",
    }:
        return "m061-acquisition-pipeline-output", "reviewed M061 acquisition pipeline output"
    if source_text in {
        "scripts/m057_marker_extract.py",
        "scripts/m058_plotextractor_extract.py",
        "scripts/m058_compare_v2_vs_m057.py",
        "scripts/m058_marker_compare_5.py",
    }:
        return "figure-extraction-benchmark-output", "reviewed figure extraction benchmark output"
    if source_text in {
        "scripts/build_m028_pdf_acquisition_diagnostics.py",
        "scripts/build_m028_universal_loader_evidence_bundles.py",
    }:
        return "m028-acquisition-evidence-output", "reviewed M028 acquisition evidence output"
    if source_text in {
        "scripts/build_r024_20_document_corpus_selection.py",
        "scripts/build_r024_53_document_corpus_selection.py",
    }:
        return "r024-corpus-selection-output", "reviewed R024 corpus selection output"
    if source_text == "scripts/extract_r024_entity_scale_entities.py":
        return "r024-entity-extraction-output", "reviewed R024 entity extraction output"
    if source_text == "scripts/convert_r024_53_pdf_to_text.py":
        return "r024-conversion-output", "reviewed R024 conversion output"
    if source_text == "scripts/build_r024_entity_networkx_probe.py":
        return "r024-networkx-probe-output", "reviewed R024 networkx probe output"
    if source_text in {
        "scripts/extract_r024_quality_metrics.py",
        "scripts/extract_r024_20_document_quality_metrics.py",
        "scripts/extract_r024_53_document_quality_metrics.py",
        "scripts/extract_r024_entity_quality_metrics.py",
    }:
        return "r024-quality-metrics-output", "reviewed R024 quality metrics output"
    if source_text == "scripts/inventory_write_paths.py":
        return "inventory-report-output", "reviewed inventory report output"
    if source_text == "scripts/soak_universal_kb_queue.py":
        return "queue-soak-output", "reviewed queue soak output"
    if source_text in {
        "scripts/verify_m072_queue_benchmark_gate.py",
        "scripts/verify_m073_queue_evidence_gate.py",
    }:
        return "queue-gate-output", "reviewed queue gate output"
    if source_text in {
        "scripts/m060g_smoke_test.py",
        "scripts/replay_m028_smoke_closeout.py",
        "scripts/run_m029_unified_loader_runtime_smoke.py",
        "scripts/verify_m029_unified_loader_runtime_smoke.py",
        "scripts/run_m122_mutation_smoke.py",
    }:
        return "smoke-script-output", "reviewed smoke script output"
    if source_text in {
        "scripts/replay_m027_current_pipeline_baseline.py",
        "scripts/replay_m027_end_to_end_mixed_replay.py",
        "scripts/synthesize_m027_pipeline_readiness.py",
        "scripts/verify_m027_provenance_and_riskratchet_gate.py",
        "scripts/verify_m027_end_to_end_mixed_replay.py",
    }:
        return "m027-pipeline-replay-output", "reviewed M027 pipeline replay output"
    if source_text in {
        "scripts/verify_m025_baseline_recovery_replay.py",
        "scripts/verify_m025_boundary_replay_completion.py",
        "scripts/verify_m025_evidence_boundaries.py",
        "scripts/verify_m025_final_preprocessing_replay.py",
        "scripts/capture_m025_article_sources.py",
    }:
        return "m025-recovery-evidence-output", "reviewed M025 recovery evidence output"
    if source_path.parts and source_path.parts[0] == "scripts":
        return "script-only", "write occurs in process-boundary script"
    if operation == "sqlite3.connect" or ".db" in path_text or "database" in path_text:
        return "database", "database-backed mutable state"
    if source_text.startswith("src/research_graph/infrastructure/graph/readiness/"):
        return "graph-readiness-evidence", "reviewed graph-readiness evidence output"
    if source_text == "src/research_graph/infrastructure/papers/source_assets/registry.py":
        return "source-asset-package", "reviewed source asset package output"
    if source_text == "src/research_graph/cli/commands/article_artifacts.py" or source_text.startswith(
        "src/research_graph/infrastructure/papers/artifacts/"
    ):
        return "article-artifact-package", "reviewed article artifact package output"
    if source_text == "src/research_graph/cli/__init__.py" and target_text in {
        "filepath",
        "day_dir / 'papers.json'",
        "day_dir / 'scored.json'",
        "day_dir / 'overview.json'",
    }:
        return "daily-cli-output", "reviewed daily CLI output"
    if source_text == "src/research_graph/infrastructure/corpus/parsing/replay_adapters.py":
        return "parser-replay-output", "reviewed parser replay output"
    if source_text in {
        "src/research_graph/infrastructure/corpus/sources/thirty_paper_deviation_scan.py",
        "src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py",
    }:
        return "source-scan-output", "reviewed source scan output"
    if source_text == "src/research_graph/infrastructure/graph/r024_networkx_probe.py":
        return "graph-probe-output", "reviewed graph probe output"
    if source_text == "src/research_graph/infrastructure/repair/chunk_baseline_measurement.py" and target_text == "index_path":
        return "caller-owned-index", "caller-provided paired review index output"
    if source_text in {
        "src/research_graph/infrastructure/repair/chunk_baseline_measurement.py",
        "src/research_graph/infrastructure/repair/chunking_benchmark.py",
    }:
        return "repair-benchmark-output", "reviewed repair benchmark output"
    if source_text == "src/research_graph/workflows/validation/batch_workflow.py" and target_text in {
        "selection_manifest_path",
        "summary_path",
        "delta_path",
        "outlier_path",
        "path",
        "output_path",
    }:
        return "validation-batch-output", "reviewed validation batch output"
    if mode and "a" in mode:
        return "append-log", "append mode"
    if any(token in path_text for token in ("events", "jsonl", "diagnostics")):
        return "append-log", "event or diagnostics log path"
    if source_text == "src/research_graph/application/validation/batch_state.py" and target_text == "output_path":
        return "run-owned-state", "workflow-owned batch state replacement"
    if source_text == "src/research_graph/infrastructure/corpus/ingestion/catalog_adapters.py" and target_text == "summary_path":
        return "legacy-evidence-regeneration", "reviewed legacy ingest summary regeneration"
    if source_text == "src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py" and target_text == "report_path":
        return "legacy-evidence-regeneration", "reviewed legacy ingest report regeneration"
    if any(token in path_text for token in ("queue", "state", "index", "catalog")):
        return "shared-state", "stable shared state or index path"
    if "temp" in target_text:
        return "temporary", "same-directory temporary write before final replacement"
    if target_text == "path" or any(token in target_text for token in CALLER_OWNED_TARGET_TOKENS):
        return "caller-owned", "caller-provided or adapter-owned output path"
    if any(token in path_text for token in ("output", "artifact", "run", "day_dir", "summary")):
        return "run-scoped", "caller/output scoped artifact path"
    return "unknown", "static scanner could not infer ownership"


class WritePathVisitor(ast.NodeVisitor):
    def __init__(self, path: Path) -> None:
        self.path = path
        self.records: list[WritePathRecord] = []

    def visit_Call(self, node: ast.Call) -> Any:
        operation: str | None = None
        target: str | None = None
        mode: str | None = None

        if isinstance(node.func, ast.Attribute) and node.func.attr in WRITE_ATTRS:
            operation = node.func.attr
            target = _unparse(node.func.value)
        elif isinstance(node.func, ast.Attribute) and node.func.attr in OPEN_ATTRS:
            mode = _mode_from_call(node)
            if mode and any(flag in mode for flag in ("w", "a", "+")):
                operation = "Path.open"
                target = _unparse(node.func.value)
        elif isinstance(node.func, ast.Name) and node.func.id == "open":
            mode = _mode_from_call(node)
            if mode and any(flag in mode for flag in ("w", "a", "+")):
                operation = "open"
                target = _unparse(node.args[0] if node.args else None)
        elif isinstance(node.func, ast.Attribute):
            call_name = _call_name(node.func)
            if call_name in {"sqlite3.connect"}:
                operation = call_name
                target = _unparse(node.args[0] if node.args else None)

        if operation and target:
            category, reason = _classify(self.path, operation, target, mode)
            self.records.append(
                WritePathRecord(
                    path=str(self.path),
                    line=node.lineno,
                    operation=operation,
                    target=target,
                    category=category,
                    reason=reason,
                )
            )
        self.generic_visit(node)


def collect_records(roots: tuple[Path, ...] = ROOTS) -> list[WritePathRecord]:
    records: list[WritePathRecord] = []
    for root in roots:
        if not root.exists():
            continue
        for path in sorted(root.rglob("*.py")):
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            visitor = WritePathVisitor(path)
            visitor.visit(tree)
            records.extend(visitor.records)
    return records


def build_payload(records: list[WritePathRecord]) -> dict[str, Any]:
    by_category = Counter(record.category for record in records)
    by_root = Counter(record.path.split("/", 1)[0] for record in records)
    return {
        "schema_version": SCHEMA_VERSION,
        "summary": {
            "total_records": len(records),
            "by_category": dict(sorted(by_category.items())),
            "by_root": dict(sorted(by_root.items())),
        },
        "records": [asdict(record) for record in records],
    }


def render_markdown(payload: dict[str, Any]) -> str:
    lines = [
        "# M167 Write Path Inventory",
        "",
        f"Schema: `{payload['schema_version']}`",
        "",
        "## Summary",
        "",
        f"Total records: `{payload['summary']['total_records']}`",
        "",
        "| Category | Count |",
        "|---|---:|",
    ]
    for category, count in payload["summary"]["by_category"].items():
        lines.append(f"| {category} | {count} |")
    lines.extend(["", "## Records", "", "| Path | Line | Operation | Target | Category |", "|---|---:|---|---|---|"])
    for record in payload["records"]:
        target = str(record["target"]).replace("|", "\\|")
        lines.append(
            f"| `{record['path']}` | {record['line']} | `{record['operation']}` | "
            f"`{target}` | {record['category']} |"
        )
    return "\n".join(lines) + "\n"


def render_delta_markdown(baseline: dict[str, Any], current: dict[str, Any]) -> str:
    baseline_summary = baseline["summary"]
    current_summary = current["summary"]
    baseline_categories = baseline_summary["by_category"]
    current_categories = current_summary["by_category"]
    categories = sorted(set(baseline_categories) | set(current_categories))
    baseline_total = int(baseline_summary["total_records"])
    current_total = int(current_summary["total_records"])
    lines = [
        "# Write Path Inventory Delta",
        "",
        f"Baseline total records: `{baseline_total}`",
        f"Current total records: `{current_total}`",
        f"Total delta: `{current_total - baseline_total:+d}`",
        "",
        "| Category | Baseline | Current | Delta |",
        "|---|---:|---:|---:|",
    ]
    for category in categories:
        baseline_count = int(baseline_categories.get(category, 0))
        current_count = int(current_categories.get(category, 0))
        lines.append(
            f"| {category} | {baseline_count} | {current_count} | "
            f"{current_count - baseline_count:+d} |"
        )
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", type=Path, required=True, help="Path for JSON inventory output")
    parser.add_argument("--markdown", type=Path, required=True, help="Path for Markdown inventory output")
    parser.add_argument("--delta-from", type=Path, help="Baseline JSON inventory for delta rendering")
    parser.add_argument("--delta-markdown", type=Path, help="Path for Markdown delta output")
    args = parser.parse_args()
    if (args.delta_from is None) != (args.delta_markdown is None):
        parser.error("--delta-from and --delta-markdown must be provided together")

    records = collect_records()
    payload = build_payload(records)
    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    args.markdown.write_text(render_markdown(payload), encoding="utf-8")
    if args.delta_from and args.delta_markdown:
        baseline = json.loads(args.delta_from.read_text(encoding="utf-8"))
        args.delta_markdown.parent.mkdir(parents=True, exist_ok=True)
        args.delta_markdown.write_text(render_delta_markdown(baseline, payload), encoding="utf-8")
    print(json.dumps(payload["summary"], sort_keys=True))


if __name__ == "__main__":
    main()
