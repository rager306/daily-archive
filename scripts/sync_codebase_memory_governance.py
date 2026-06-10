#!/usr/bin/env python3
"""Generate codebase-memory governance mirror artifacts from canonical GSD state.

Generated outputs are intentionally mirrors, not sources of truth. Canonical
requirement and decision lifecycle stays in `.gsd/`; canonical architecture
rationale stays in `doc/adr/`; GitNexus remains the code-safety layer.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / ".codebase-memory" / "adr.md"
DEFAULT_GRAPH_OUTPUT = ROOT / ".codebase-memory" / "governance-graph.json"
REQUIREMENTS_PATH = ROOT / ".gsd" / "REQUIREMENTS.md"
DECISIONS_PATH = ROOT / ".gsd" / "DECISIONS.md"
ADR_DIR = ROOT / "doc" / "adr" / "m034"

SECRET_PATTERNS = (
    re.compile(r"sk-[A-Za-z0-9_-]{12,}"),
    re.compile(r"(?i)(api[_-]?key|secret|token|credential)\s*[:=]\s*[^\s`|]+"),
    re.compile(r"(?i)-----BEGIN [A-Z ]*PRIVATE KEY-----"),
)
FORBIDDEN_PAYLOAD_TERMS = (
    "raw_corpus_text",
    "raw_prompt",
    "full_text",
    "source_text",
    "embedding_vector",
)
REQUIRED_GRAPH_NODES = frozenset({"D075", "D076", "R062", "R063", "ADR-005", "M038", "M039"})
REQUIRED_GRAPH_EDGES = frozenset(
    {
        ("D076", "extends", "D075"),
        ("D076", "implements", "R063"),
        ("R063", "owned_by", "M039"),
        ("R062", "validated_by", "M038"),
        ("ADR-005", "blocks", "SAFETY-NO-DIRECT-GRAPHDB-WRITES"),
        ("M038", "provides", "ARTIFACT-CODEBASE-MEMORY-ADR-MIRROR"),
        ("M039", "provides", "ARTIFACT-GOVERNANCE-GRAPH"),
    }
)


@dataclass(frozen=True)
class RequirementEntry:
    req_id: str
    title: str
    status: str
    source: str
    owner: str


@dataclass(frozen=True)
class DecisionEntry:
    decision_id: str
    when: str
    scope: str
    decision: str
    choice: str


@dataclass(frozen=True)
class AdrEntry:
    adr_id: str
    title: str
    status: str
    path: str


@dataclass(frozen=True)
class GraphNode:
    id: str
    type: str
    title: str
    canonical_source: str
    status: str = "active"
    mirror_only: bool = True


@dataclass(frozen=True)
class GraphEdge:
    source: str
    relationship: str
    target: str
    rationale: str


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def clean_cell(value: str) -> str:
    value = value.strip()
    value = re.sub(r"\s+", " ", value)
    return value.replace("`", "'")


def check_safe_text(text: str) -> None:
    for pattern in SECRET_PATTERNS:
        if pattern.search(text):
            raise ValueError("governance mirror contains secret-shaped content")
    lowered = text.lower()
    for term in FORBIDDEN_PAYLOAD_TERMS:
        if term in lowered:
            raise ValueError(f"governance mirror contains forbidden payload term: {term}")


def parse_requirements(text: str) -> list[RequirementEntry]:
    entries: list[RequirementEntry] = []
    blocks = re.split(r"(?=^### R\d{3}\s+[—-]\s+)", text, flags=re.MULTILINE)
    for block in blocks:
        header = re.match(r"^### (R\d{3})\s+[—-]\s+(.+)$", block.strip(), flags=re.MULTILINE)
        if not header:
            continue
        req_id, title = header.group(1), clean_cell(header.group(2))
        status = extract_bullet(block, "Status") or "unknown"
        source = extract_bullet(block, "Source") or "unknown"
        owner = extract_bullet(block, "Primary owning slice") or "unassigned"
        entries.append(RequirementEntry(req_id, title, clean_cell(status), clean_cell(source), clean_cell(owner)))
    return entries


def parse_decisions(text: str) -> list[DecisionEntry]:
    entries: list[DecisionEntry] = []
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line.startswith("| D"):
            continue
        cells = [clean_cell(cell) for cell in line.strip("|").split("|")]
        if len(cells) < 8:
            continue
        entries.append(
            DecisionEntry(
                decision_id=cells[0],
                when=cells[1],
                scope=cells[2],
                decision=cells[3],
                choice=cells[4],
            )
        )
    return entries


def parse_adrs(adr_dir: Path) -> list[AdrEntry]:
    entries: list[AdrEntry] = []
    for path in sorted(adr_dir.glob("ADR-*.md")):
        text = read_text(path)
        header = re.search(r"^#\s+(ADR-\d+):\s+(.+)$", text, flags=re.MULTILINE)
        if not header:
            continue
        status = "unknown"
        status_match = re.search(r"^\*\*Status:\*\*\s*(.+?)\s*$", text, flags=re.MULTILINE)
        if status_match:
            status = clean_cell(status_match.group(1))
        entries.append(
            AdrEntry(
                adr_id=header.group(1),
                title=clean_cell(header.group(2)),
                status=status,
                path=str(path.relative_to(ROOT)),
            )
        )
    return entries


def extract_bullet(block: str, name: str) -> str | None:
    match = re.search(rf"^- {re.escape(name)}:\s*(.+)$", block, flags=re.MULTILINE)
    return match.group(1).strip() if match else None


def render_digest(
    requirements: list[RequirementEntry], decisions: list[DecisionEntry], adrs: list[AdrEntry]
) -> str:
    lines: list[str] = [
        "# daily-archive Governance Memory Mirror",
        "",
        "> Generated by `scripts/sync_codebase_memory_governance.py`.",
        "> GSD remains canonical for requirements and decisions; documented ADR files remain canonical for architecture decisions; GitNexus remains mandatory for code-impact safety.",
        "> This file is a compact codebase-memory recall mirror only. If it conflicts with `.gsd/` or `doc/adr/`, treat this file as stale and regenerate it.",
        "",
        "## Hybrid Governance Roles",
        "",
        "| Layer | Role | Canonical? |",
        "|---|---|---:|",
        "| GSD `.gsd/REQUIREMENTS.md` and `.gsd/DECISIONS.md` | Requirement and decision lifecycle | yes |",
        "| ADR docs under `doc/adr/` | Architecture decision rationale and binding notes | yes |",
        "| GitNexus | Pre-edit impact analysis and pre-commit change scope | yes for code safety workflow |",
        "| codebase-memory MCP | Fast semantic ADR/R/D recall mirror | no |",
        "",
        "## Requirement Index",
        "",
        "| ID | Status | Owner | Source | Title |",
        "|---|---|---|---|---|",
    ]
    for req in requirements:
        lines.append(f"| {req.req_id} | {req.status} | {req.owner} | {req.source} | {req.title} |")

    lines.extend([
        "",
        "## Decision Index",
        "",
        "| ID | When | Scope | Decision | Choice |",
        "|---|---|---|---|---|",
    ])
    for decision in decisions:
        lines.append(
            f"| {decision.decision_id} | {decision.when} | {decision.scope} | {decision.decision} | {decision.choice} |"
        )

    lines.extend([
        "",
        "## ADR Index",
        "",
        "| ADR | Status | Path | Title |",
        "|---|---|---|---|",
    ])
    for adr in adrs:
        lines.append(f"| {adr.adr_id} | {adr.status} | `{adr.path}` | {adr.title} |")

    lines.extend([
        "",
        "## Typed Graph Projection",
        "",
        "The graph-shaped projection lives in `.codebase-memory/governance-graph.json`. It is generated mirror state, not canonical state, and exists for codebase-memory search/readback and future typed ingestion once supported.",
        "",
        "## ADR Relationship Graph Notes",
        "",
        "- ADR-000 establishes the Universal KB north-star scope and prevents overfitting to arXiv, PDFs, scientific papers, or RAG-only assumptions.",
        "- ADR-004 allows sidecars to produce candidate evidence, but not import-ready graph facts.",
        "- ADR-005 blocks direct extractor, parser, sidecar, adapter, or LLM-helper writes to GraphDB and requires candidate, validation, review, and readiness boundaries before any promotion.",
        "- M035 validates the no-write Universal KB prototype at fixture level.",
        "- M036 validates the no-write smoke on 5 real local article artifacts.",
        "- M037 consolidates the smoke control surface without changing ADR-005 or expanding beyond 5 articles.",
        "- D075 chooses the hybrid governance model: GSD canonical, GitNexus code-safety, codebase-memory fast recall mirror.",
        "- D076 adds typed graph projection artifacts because codebase-memory MCP custom edge ingestion is not yet implemented.",
        "- R062 requires this generated mirror to stay non-canonical and verifiable.",
        "- R063 requires the typed graph projection to expose verifiable ADR/R/D nodes and edges while preserving source-of-truth boundaries.",
        "",
        "## Refresh Commands",
        "",
        "```bash",
        "uv run python scripts/sync_codebase_memory_governance.py",
        "uv run python scripts/sync_codebase_memory_governance.py --check",
        "```",
        "",
    ])
    rendered = "\n".join(lines)
    check_safe_text(rendered)
    return rendered


def graph_node(node_id: str, node_type: str, title: str, source: str, status: str = "active") -> GraphNode:
    return GraphNode(id=node_id, type=node_type, title=title, canonical_source=source, status=status)


def build_graph(
    requirements: list[RequirementEntry], decisions: list[DecisionEntry], adrs: list[AdrEntry]
) -> dict[str, Any]:
    nodes: dict[str, GraphNode] = {}
    edges: list[GraphEdge] = []

    for req in requirements:
        nodes[req.req_id] = graph_node(req.req_id, "Requirement", req.title, ".gsd/REQUIREMENTS.md", req.status)
    for decision in decisions:
        nodes[decision.decision_id] = graph_node(
            decision.decision_id,
            "Decision",
            decision.decision,
            ".gsd/DECISIONS.md",
            "recorded",
        )
    for adr in adrs:
        nodes[adr.adr_id] = graph_node(adr.adr_id, "ADR", adr.title, adr.path, adr.status)

    milestone_titles = {
        "M035": "Universal KB no-write prototype",
        "M036": "Real-corpus no-write smoke",
        "M037": "Universal KB control surface consolidation",
        "M038": "Governance memory bridge",
        "M039": "Typed governance graph projection",
    }
    for milestone_id, title in milestone_titles.items():
        nodes[milestone_id] = graph_node(milestone_id, "Milestone", title, ".gsd/milestones", "historical")

    artifact_titles = {
        "ARTIFACT-CODEBASE-MEMORY-ADR-MIRROR": ".codebase-memory/adr.md",
        "ARTIFACT-GOVERNANCE-GRAPH": ".codebase-memory/governance-graph.json",
    }
    for artifact_id, title in artifact_titles.items():
        nodes[artifact_id] = graph_node(artifact_id, "Artifact", title, title, "generated")

    nodes["SAFETY-NO-DIRECT-GRAPHDB-WRITES"] = graph_node(
        "SAFETY-NO-DIRECT-GRAPHDB-WRITES",
        "SafetyBoundary",
        "No direct extractor/parser/sidecar/adapter/LLM helper writes to GraphDB",
        "doc/adr/m034/ADR-005-no-direct-extractor-to-graphdb-path.md",
        "binding",
    )

    def add(source: str, relationship: str, target: str, rationale: str) -> None:
        if source in nodes and target in nodes:
            edges.append(GraphEdge(source, relationship, target, rationale))

    add("D076", "extends", "D075", "Typed projection extends the hybrid governance-memory model.")
    add("D076", "implements", "R063", "D076 chooses artifact-first typed projection for R063.")
    add("D075", "implements", "R062", "D075 defines the non-canonical mirror model required by R062.")
    add("R063", "owned_by", "M039", "M039 owns typed governance graph projection delivery.")
    add("R062", "validated_by", "M038", "M038 validated the generated ADR/R/D mirror.")
    add("M038", "provides", "ARTIFACT-CODEBASE-MEMORY-ADR-MIRROR", "M038 generated the markdown recall mirror.")
    add("M039", "provides", "ARTIFACT-GOVERNANCE-GRAPH", "M039 generates the typed graph projection.")
    add("ARTIFACT-GOVERNANCE-GRAPH", "mirrors", "ARTIFACT-CODEBASE-MEMORY-ADR-MIRROR", "Both artifacts are generated from canonical GSD/ADR inputs.")
    add("ADR-005", "blocks", "SAFETY-NO-DIRECT-GRAPHDB-WRITES", "ADR-005 is the binding no direct GraphDB path rule.")
    add("SAFETY-NO-DIRECT-GRAPHDB-WRITES", "constrains", "R063", "The typed graph projection must remain metadata-only and non-promotional.")
    add("M035", "validates", "ADR-005", "M035 validates the no-write rule at fixture level.")
    add("M036", "validates", "ADR-005", "M036 validates the no-write rule on 5 real local article artifacts.")
    add("M037", "preserves", "ADR-005", "M037 consolidates controls without weakening no-write constraints.")

    graph = {
        "schema_version": "governance-graph/v1",
        "generated_by": "scripts/sync_codebase_memory_governance.py",
        "mirror_only": True,
        "canonical_sources": {
            "requirements": str(REQUIREMENTS_PATH.relative_to(ROOT)),
            "decisions": str(DECISIONS_PATH.relative_to(ROOT)),
            "adrs": str(ADR_DIR.relative_to(ROOT)),
        },
        "source_of_truth_warning": "GSD remains canonical for requirements and decisions; ADR docs remain canonical for architecture; this JSON is a codebase-memory recall projection only.",
        "nodes": [asdict(nodes[node_id]) for node_id in sorted(nodes)],
        "edges": [asdict(edge) for edge in sorted(edges, key=lambda edge: (edge.source, edge.relationship, edge.target))],
    }
    validate_graph(graph)
    return graph


def validate_graph(graph: dict[str, Any]) -> None:
    serialized = json.dumps(graph, sort_keys=True, ensure_ascii=False)
    check_safe_text(serialized)
    if graph.get("mirror_only") is not True:
        raise ValueError("governance graph must be marked mirror_only=true")
    node_ids = {node["id"] for node in graph.get("nodes", [])}
    missing_nodes = REQUIRED_GRAPH_NODES - node_ids
    if missing_nodes:
        raise ValueError(f"governance graph missing required nodes: {sorted(missing_nodes)}")
    edge_keys = {
        (edge["source"], edge["relationship"], edge["target"]) for edge in graph.get("edges", [])
    }
    missing_edges = REQUIRED_GRAPH_EDGES - edge_keys
    if missing_edges:
        raise ValueError(f"governance graph missing required edges: {sorted(missing_edges)}")


def load_entries() -> tuple[list[RequirementEntry], list[DecisionEntry], list[AdrEntry]]:
    project_adr_dir = ROOT / "doc" / "adr"
    seen: dict[str, AdrEntry] = {}
    for adr_entry in parse_adrs(ADR_DIR):
        seen[adr_entry.adr_id] = adr_entry
    for adr_entry in parse_adrs(project_adr_dir):
        existing = seen.get(adr_entry.adr_id)
        if existing is None or (adr_entry.path and not existing.path.startswith("doc/adr/")):
            seen[adr_entry.adr_id] = adr_entry
    return (
        parse_requirements(read_text(REQUIREMENTS_PATH)),
        parse_decisions(read_text(DECISIONS_PATH)),
        list(seen.values()),
    )


def generate_digest() -> str:
    requirements, decisions, adrs = load_entries()
    return render_digest(requirements, decisions, adrs)


def generate_graph() -> dict[str, Any]:
    requirements, decisions, adrs = load_entries()
    return build_graph(requirements, decisions, adrs)


def render_graph(graph: dict[str, Any]) -> str:
    return json.dumps(graph, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def write_outputs(output: Path, graph_output: Path) -> None:
    digest = generate_digest()
    graph = render_graph(generate_graph())
    output.parent.mkdir(parents=True, exist_ok=True)
    graph_output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(digest, encoding="utf-8")
    graph_output.write_text(graph, encoding="utf-8")
    sys.stdout.write(f"wrote {display_path(output)}\n")
    sys.stdout.write(f"wrote {display_path(graph_output)}\n")


def check_outputs(output: Path, graph_output: Path) -> None:
    expected_digest = generate_digest()
    expected_graph = render_graph(generate_graph())
    actual_digest = read_text(output) if output.exists() else ""
    actual_graph = read_text(graph_output) if graph_output.exists() else ""
    stale: list[str] = []
    if actual_digest != expected_digest:
        stale.append(display_path(output))
    if actual_graph != expected_graph:
        stale.append(display_path(graph_output))
    if stale:
        raise SystemExit(f"stale governance mirror artifacts: {', '.join(stale)}; run sync_codebase_memory_governance.py")
    sys.stdout.write(f"ok {display_path(output)}\n")
    sys.stdout.write(f"ok {display_path(graph_output)}\n")


# Compatibility helpers used by tests and older task summaries.
def write_digest(output: Path) -> None:
    write_outputs(output, DEFAULT_GRAPH_OUTPUT)


def check_digest(output: Path) -> None:
    check_outputs(output, DEFAULT_GRAPH_OUTPUT)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if generated artifacts are stale")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--graph-output", type=Path, default=DEFAULT_GRAPH_OUTPUT)
    args = parser.parse_args(argv)

    output = args.output.resolve()
    graph_output = args.graph_output.resolve()
    if args.check:
        check_outputs(output, graph_output)
    else:
        write_outputs(output, graph_output)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ValueError as exc:
        sys.stderr.write(f"error: {exc}\n")
        raise SystemExit(1) from exc
