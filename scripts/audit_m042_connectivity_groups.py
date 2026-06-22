#!/usr/bin/env python3
"""Audit M041/M042 metadata-only connectivity groups without graph writes."""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict, deque
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "artifacts" / "m041-mixed-connectivity-smoke" / "manifest.json"
DEFAULT_REPAIR_REPORT = ROOT / "artifacts" / "m042-linked-metadata-readiness" / "repair-report.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m042-linked-metadata-readiness"
FALSE_SAFETY_KEYS = (
    "graph_write_allowed",
    "import_eligible",
    "production_import_attempted",
    "promotion_allowed",
)


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def infer_selected_source_key(evidence_ref: str, selected_keys: set[str]) -> str | None:
    for key in sorted(selected_keys, key=len, reverse=True):
        if f"/{key}/" in evidence_ref or evidence_ref.endswith(f"/{key}"):
            return key
    return None


def build_nodes(
    articles: list[dict[str, Any]], repair_report: dict[str, Any]
) -> list[dict[str, Any]]:
    repair_by_key = {record["article_key"]: record for record in repair_report.get("records", [])}
    nodes: list[dict[str, Any]] = []
    for entry in articles:
        key = str(entry.get("article_key"))
        repair = repair_by_key.get(key, {})
        nodes.append(
            {
                "article_key": key,
                "category": entry.get("m041_category"),
                "catalog_path": entry.get("catalog_path"),
                "metadata_status": repair.get("after_status")
                or entry.get("metadata_status")
                or "not_applicable",
                "connectivity_role": entry.get("connectivity_role", ""),
            }
        )
    return nodes


def build_evidence_edges(
    articles: list[dict[str, Any]], selected_keys: set[str]
) -> list[dict[str, Any]]:
    edges: list[dict[str, Any]] = []
    for entry in articles:
        if entry.get("m041_category") != "reference_linked":
            continue
        target = str(entry.get("article_key"))
        linked_from = entry.get("linked_from") or []
        if not isinstance(linked_from, list):
            linked_from = [linked_from]
        for evidence_ref in linked_from:
            evidence = str(evidence_ref)
            source_key = infer_selected_source_key(evidence, selected_keys)
            edges.append(
                {
                    "kind": "local_reference",
                    "source": source_key,
                    "source_ref": evidence,
                    "target": target,
                    "target_category": entry.get("m041_category"),
                    "connects_selected_nodes": source_key in selected_keys
                    and target in selected_keys,
                }
            )
    return edges


def connected_components(
    nodes: list[dict[str, Any]], edges: list[dict[str, Any]]
) -> list[list[str]]:
    selected_keys = {node["article_key"] for node in nodes}
    adjacency: dict[str, set[str]] = defaultdict(set)
    for key in selected_keys:
        adjacency[key]
    for edge in edges:
        source = edge.get("source")
        target = edge.get("target")
        if (
            edge.get("connects_selected_nodes")
            and isinstance(source, str)
            and isinstance(target, str)
        ):
            adjacency[source].add(target)
            adjacency[target].add(source)

    seen: set[str] = set()
    components: list[list[str]] = []
    for key in sorted(selected_keys):
        if key in seen:
            continue
        queue: deque[str] = deque([key])
        seen.add(key)
        component: list[str] = []
        while queue:
            current = queue.popleft()
            component.append(current)
            for neighbor in sorted(adjacency[current]):
                if neighbor not in seen:
                    seen.add(neighbor)
                    queue.append(neighbor)
        components.append(sorted(component))
    return sorted(components, key=lambda component: (-len(component), component[0]))


def audit_connectivity(
    *, manifest_path: Path, repair_report_path: Path, output_dir: Path
) -> dict[str, Any]:
    manifest = load_json(manifest_path)
    repair_report = load_json(repair_report_path)
    articles = manifest.get("articles")
    if not isinstance(articles, list):
        raise ValueError("manifest articles must be a list")
    safety_flags = (
        manifest.get("safety_flags") if isinstance(manifest.get("safety_flags"), dict) else {}
    )
    if any(safety_flags.get(key) is not False for key in FALSE_SAFETY_KEYS):
        raise ValueError("M041 manifest safety flags must remain false")

    nodes = build_nodes(articles, repair_report)
    selected_keys = {node["article_key"] for node in nodes}
    edges = build_evidence_edges(articles, selected_keys)
    components = connected_components(nodes, edges)
    isolated = [component[0] for component in components if len(component) == 1]
    hermes_group = sorted(
        node["article_key"] for node in nodes if node["category"] == "hermes_review_section"
    )
    category_counts: dict[str, int] = {}
    for node in nodes:
        category = str(node["category"])
        category_counts[category] = category_counts.get(category, 0) + 1

    audit = {
        "source_manifest": str(manifest_path),
        "repair_report": str(repair_report_path),
        "node_count": len(nodes),
        "category_counts": category_counts,
        "nodes": nodes,
        "evidence_edges": edges,
        "edge_counts": {
            "local_reference": sum(1 for edge in edges if edge["kind"] == "local_reference"),
            "selected_node_edges": sum(1 for edge in edges if edge["connects_selected_nodes"]),
        },
        "components": [
            {"size": len(component), "article_keys": component} for component in components
        ],
        "component_count": len(components),
        "largest_component_size": len(components[0]) if components else 0,
        "isolated_article_count": len(isolated),
        "isolated_articles": isolated,
        "hermes_co_selection_group": {
            "article_count": len(hermes_group),
            "article_keys": hermes_group,
            "counts_as_reference_edges": False,
        },
        "safety_flags": safety_flags,
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }
    write_json(output_dir / "connectivity-audit.json", audit)
    write_text(output_dir / "connectivity-audit.md", render_markdown(audit))
    return audit


def render_markdown(audit: dict[str, Any]) -> str:
    lines = [
        "# M042 Connectivity Group Audit",
        "",
        f"- Node count: {audit['node_count']}",
        f"- Category counts: {audit['category_counts']}",
        f"- Local reference edges: {audit['edge_counts']['local_reference']}",
        f"- Selected-node evidence edges: {audit['edge_counts']['selected_node_edges']}",
        f"- Component count: {audit['component_count']}",
        f"- Largest component size: {audit['largest_component_size']}",
        f"- Isolated articles: {audit['isolated_article_count']}",
        "- Hermes co-selection counts as reference edges: false",
        "- Graph writes: disabled",
        "- Production import: disabled",
        "- Fact promotion: disabled",
        "",
        "## Components",
        "",
        "| Size | Article keys |",
        "|---:|---|",
    ]
    for component in audit["components"]:
        lines.append(f"| {component['size']} | {', '.join(component['article_keys'])} |")
    lines.extend(
        [
            "",
            "## Evidence edges",
            "",
            "| Kind | Source | Target | Evidence |",
            "|---|---|---|---|",
        ]
    )
    for edge in audit["evidence_edges"]:
        source = edge.get("source") or "external_or_unselected"
        lines.append(f"| {edge['kind']} | {source} | {edge['target']} | `{edge['source_ref']}` |")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--repair-report", type=Path, default=DEFAULT_REPAIR_REPORT)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    args = parser.parse_args()
    audit = audit_connectivity(
        manifest_path=args.manifest,
        repair_report_path=args.repair_report,
        output_dir=args.output_dir,
    )
    sys.stdout.write(
        "m042 connectivity audit complete: "
        f"nodes={audit['node_count']} local_edges={audit['edge_counts']['local_reference']} "
        f"components={audit['component_count']} largest={audit['largest_component_size']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
