#!/usr/bin/env python3
"""Emit the M060c S02 graph-library applicability matrix.

The matrix is a decision artifact, not a runtime integration. It keeps
NetworkX as the canonical graph layer while binding igraph and rustworkx as
supplementary accelerators for heavy read-only algorithms.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m060c-benchmark"
LOOPBACK_HOST = "127.0.0.1"

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_enabled": False,
    "llm_calls_enabled": False,
}

SAFETY_STATEMENTS = [
    "Graph writes are not authorized.",
    "Production import is not authorized.",
    "Fact promotion is not authorized.",
    "External network default is disabled.",
    "LLM calls default is disabled.",
]

LIBRARIES = [
    "NetworkX",
    "igraph",
    "rustworkx",
    "graph-tool",
    "PyG",
    "DGL",
    "NetworkX-Temporal",
    "GraphScope",
]

MILESTONES = [
    "M060b (intermediate layer)",
    "M061 (2-hop BFS)",
    "M062 (fd hardening)",
    "M063 (GraphDB selection)",
    "M064+ (production)",
]

RESEARCH_REFERENCES = {
    "NetworkX": "artifacts/m060c-benchmark/benchmark.json",
    "igraph": "artifacts/m060c-benchmark/library-research/python-igraph.md",
    "rustworkx": "artifacts/m060c-benchmark/library-research/rustworkx.md",
    "graph-tool": "artifacts/m060c-benchmark/library-research/graph-tool.md",
    "PyG": "artifacts/m060c-benchmark/library-research/pytorch_geometric.md",
    "DGL": "artifacts/m060c-benchmark/library-research/dgl.md",
    "NetworkX-Temporal": "artifacts/m060c-benchmark/library-research/networkx-temporal.md",
    "GraphScope": "artifacts/m060c-benchmark/library-research/graphscope.md",
}

# applicability_score: 0 = avoid now, 1 = niche/deferred, 2 = useful, 3 = preferred.
CELL_DATA: dict[str, dict[str, dict[str, Any]]] = {
    "NetworkX": {
        "M060b (intermediate layer)": {
            "applicability_score": 3,
            "use_case_fit": "Canonical authoring and readable control graph for the intermediate layer.",
            "integration_cost": "Low; already used as the baseline and requires no new dependency.",
            "decision": "Keep as primary graph representation.",
        },
        "M061 (2-hop BFS)": {
            "applicability_score": 2,
            "use_case_fit": "Reliable correctness baseline for 2-hop BFS and regression checks.",
            "integration_cost": "Low; slower than igraph/rustworkx on heavy operations.",
            "decision": "Use as control path; accelerate only measured hot spots.",
        },
        "M062 (fd hardening)": {
            "applicability_score": 3,
            "use_case_fit": "Best fit for deterministic read-only hardening and reviewable diagnostics.",
            "integration_cost": "Low; mature API and no conversion boundary.",
            "decision": "Use as primary library.",
        },
        "M063 (GraphDB selection)": {
            "applicability_score": 2,
            "use_case_fit": "Portable reference model for comparing GraphDB candidates.",
            "integration_cost": "Low; does not solve persistence or query substrate choice.",
            "decision": "Use as benchmark harness and semantic control.",
        },
        "M064+ (production)": {
            "applicability_score": 2,
            "use_case_fit": "Safe control-plane graph for production checks, not the only scaling path.",
            "integration_cost": "Low; performance limits remain for larger algorithm-heavy jobs.",
            "decision": "Keep primary for read-only control operations.",
        },
    },
    "igraph": {
        "M060b (intermediate layer)": {
            "applicability_score": 3,
            "use_case_fit": "Strong supplementary backend for PageRank/components and other algorithm-heavy reads.",
            "integration_cost": "Medium; requires conversion from the NetworkX/control representation.",
            "decision": "Adopt as supplementary accelerator.",
        },
        "M061 (2-hop BFS)": {
            "applicability_score": 3,
            "use_case_fit": "Measured 5-10x-class speedups on heavy operations, with especially strong PageRank/components results.",
            "integration_cost": "Medium; keep NetworkX parity tests around conversion.",
            "decision": "Use for algorithm-heavy 2-hop BFS adjacent analysis where benchmarks justify it.",
        },
        "M062 (fd hardening)": {
            "applicability_score": 2,
            "use_case_fit": "Useful for heavy diagnostic scans, but not needed for authoring or safety gates.",
            "integration_cost": "Medium; conversion adds another failure surface.",
            "decision": "Use only for measured hot paths.",
        },
        "M063 (GraphDB selection)": {
            "applicability_score": 2,
            "use_case_fit": "Good in-process comparator before choosing an external GraphDB substrate.",
            "integration_cost": "Medium; remains an algorithm library, not a database.",
            "decision": "Use as benchmark comparator, not as GraphDB replacement.",
        },
        "M064+ (production)": {
            "applicability_score": 2,
            "use_case_fit": "Candidate production accelerator after explicit performance and packaging proof.",
            "integration_cost": "Medium; binary packaging and parity checks required.",
            "decision": "Allow as supplementary read-only accelerator after gate approval.",
        },
    },
    "rustworkx": {
        "M060b (intermediate layer)": {
            "applicability_score": 2,
            "use_case_fit": "Useful low-latency traversal/path backend for selected heavy reads.",
            "integration_cost": "Medium; less direct authoring ergonomics than NetworkX.",
            "decision": "Adopt as optional supplementary accelerator.",
        },
        "M061 (2-hop BFS)": {
            "applicability_score": 3,
            "use_case_fit": "Strong fit for BFS and shortest-path hot spots when available.",
            "integration_cost": "Medium; conversion and parity checks required.",
            "decision": "Use for BFS/path hot paths if local availability remains stable.",
        },
        "M062 (fd hardening)": {
            "applicability_score": 1,
            "use_case_fit": "Niche fit for traversal diagnostics only.",
            "integration_cost": "Medium; not worth broadening unless a hot spot appears.",
            "decision": "Defer except for measured traversal bottlenecks.",
        },
        "M063 (GraphDB selection)": {
            "applicability_score": 1,
            "use_case_fit": "Useful performance comparator, but not a GraphDB substrate.",
            "integration_cost": "Medium; does not address persistence/query requirements.",
            "decision": "Use only in benchmark comparisons.",
        },
        "M064+ (production)": {
            "applicability_score": 2,
            "use_case_fit": "Production accelerator candidate for traversal/path workloads.",
            "integration_cost": "Medium; Rust extension packaging and fallback path required.",
            "decision": "Allow as optional read-only accelerator after gate approval.",
        },
    },
    "graph-tool": {
        "M060b (intermediate layer)": {
            "applicability_score": 0,
            "use_case_fit": "Potentially fast but not vendored or source-verified in S01.",
            "integration_cost": "High; conda/system-package friction is disproportionate now.",
            "decision": "Do not adopt.",
        },
        "M061 (2-hop BFS)": {
            "applicability_score": 1,
            "use_case_fit": "May be valuable only if pip-installable accelerators miss latency targets.",
            "integration_cost": "High; runtime packaging risk remains unresolved.",
            "decision": "Defer pending scale failure evidence.",
        },
        "M062 (fd hardening)": {
            "applicability_score": 0,
            "use_case_fit": "No hardening benefit over already available libraries.",
            "integration_cost": "High.",
            "decision": "Do not use.",
        },
        "M063 (GraphDB selection)": {
            "applicability_score": 1,
            "use_case_fit": "Could be a performance reference, not a GraphDB decision.",
            "integration_cost": "High.",
            "decision": "Revisit only as a later benchmark candidate.",
        },
        "M064+ (production)": {
            "applicability_score": 1,
            "use_case_fit": "Possible future high-performance backend if packaging becomes acceptable.",
            "integration_cost": "High; deployment complexity blocks adoption now.",
            "decision": "Deferred.",
        },
    },
    "PyG": {
        "M060b (intermediate layer)": {
            "applicability_score": 0,
            "use_case_fit": "GNN/tensor workflow mismatch for deterministic graph diagnostics.",
            "integration_cost": "High; model/data-loader stack is unnecessary.",
            "decision": "Do not adopt.",
        },
        "M061 (2-hop BFS)": {
            "applicability_score": 0,
            "use_case_fit": "Not a direct BFS/read-only graph analytics surface.",
            "integration_cost": "High.",
            "decision": "Do not use.",
        },
        "M062 (fd hardening)": {
            "applicability_score": 0,
            "use_case_fit": "No fit for fd hardening.",
            "integration_cost": "High.",
            "decision": "Do not use.",
        },
        "M063 (GraphDB selection)": {
            "applicability_score": 1,
            "use_case_fit": "Only relevant if a future GNN requirement appears.",
            "integration_cost": "High.",
            "decision": "Defer.",
        },
        "M064+ (production)": {
            "applicability_score": 1,
            "use_case_fit": "Possible future ML layer, not current graph substrate.",
            "integration_cost": "High.",
            "decision": "Out of scope until ML requirement exists.",
        },
    },
    "DGL": {
        "M060b (intermediate layer)": {
            "applicability_score": 0,
            "use_case_fit": "Deep-learning graph framework is mismatched for lightweight read-only analytics.",
            "integration_cost": "High; dependency and data-model overhead.",
            "decision": "Do not adopt.",
        },
        "M061 (2-hop BFS)": {
            "applicability_score": 0,
            "use_case_fit": "Not a clean replacement for deterministic BFS diagnostics.",
            "integration_cost": "High.",
            "decision": "Do not use.",
        },
        "M062 (fd hardening)": {
            "applicability_score": 0,
            "use_case_fit": "No direct fit for fd hardening.",
            "integration_cost": "High.",
            "decision": "Do not use.",
        },
        "M063 (GraphDB selection)": {
            "applicability_score": 1,
            "use_case_fit": "Relevant only for a future GNN/heterograph evaluation.",
            "integration_cost": "High.",
            "decision": "Defer.",
        },
        "M064+ (production)": {
            "applicability_score": 1,
            "use_case_fit": "Possible future ML substrate, not current production graph layer.",
            "integration_cost": "High.",
            "decision": "Out of scope until ML requirement exists.",
        },
    },
    "NetworkX-Temporal": {
        "M060b (intermediate layer)": {
            "applicability_score": 1,
            "use_case_fit": "Conceptually adjacent, but current graph is typed evidence rather than time-sliced state.",
            "integration_cost": "Low-to-medium; extends NetworkX but adds premature modeling.",
            "decision": "Defer.",
        },
        "M061 (2-hop BFS)": {
            "applicability_score": 0,
            "use_case_fit": "No direct acceleration for 2-hop BFS.",
            "integration_cost": "Medium.",
            "decision": "Do not use.",
        },
        "M062 (fd hardening)": {
            "applicability_score": 1,
            "use_case_fit": "Could model temporal hardening later, but not needed now.",
            "integration_cost": "Medium.",
            "decision": "Defer.",
        },
        "M063 (GraphDB selection)": {
            "applicability_score": 1,
            "use_case_fit": "Temporal semantics may inform future requirements, not substrate selection now.",
            "integration_cost": "Medium.",
            "decision": "Defer.",
        },
        "M064+ (production)": {
            "applicability_score": 1,
            "use_case_fit": "Possible future temporal layer if requirements become time-sliced.",
            "integration_cost": "Medium.",
            "decision": "Defer until temporal requirement is explicit.",
        },
    },
    "GraphScope": {
        "M060b (intermediate layer)": {
            "applicability_score": 0,
            "use_case_fit": "Distributed graph system is too heavy for the intermediate layer.",
            "integration_cost": "High; operational footprint is disproportionate.",
            "decision": "Do not adopt.",
        },
        "M061 (2-hop BFS)": {
            "applicability_score": 0,
            "use_case_fit": "Distributed execution is unnecessary for current 2-hop BFS scale.",
            "integration_cost": "High.",
            "decision": "Do not use.",
        },
        "M062 (fd hardening)": {
            "applicability_score": 0,
            "use_case_fit": "No fit for fd hardening.",
            "integration_cost": "High.",
            "decision": "Do not use.",
        },
        "M063 (GraphDB selection)": {
            "applicability_score": 2,
            "use_case_fit": "Potentially relevant only if GraphDB selection requires distributed analytics comparison.",
            "integration_cost": "High; repo was not available in GitNexus during S01.",
            "decision": "Evaluate as a candidate only during GraphDB selection.",
        },
        "M064+ (production)": {
            "applicability_score": 1,
            "use_case_fit": "Future distributed option if single-process libraries fail production scale.",
            "integration_cost": "High.",
            "decision": "Defer until production scale proves need.",
        },
    },
}


def build_m060c_s02_applicability_matrix() -> dict[str, Any]:
    """Return the full S02 applicability matrix payload."""
    cells: list[dict[str, Any]] = []
    for library in LIBRARIES:
        for milestone in MILESTONES:
            cell = CELL_DATA[library][milestone]
            cells.append(
                {
                    "library": library,
                    "milestone": milestone,
                    "applicability_score": cell["applicability_score"],
                    "use_case_fit": cell["use_case_fit"],
                    "integration_cost": cell["integration_cost"],
                    "decision": cell["decision"],
                    "research_reference": RESEARCH_REFERENCES[library],
                }
            )

    aggregate = {
        library: sum(
            1
            for cell in cells
            if cell["library"] == library and cell["applicability_score"] >= 2
        )
        for library in LIBRARIES
    }

    return {
        "schema_version": 1,
        "artifact": "m060c_s02_applicability_matrix",
        "milestone": "M061-0fib2i",
        "slice": "S02",
        "loopback_host": LOOPBACK_HOST,
        "libraries": LIBRARIES,
        "milestones": MILESTONES,
        "cells": cells,
        "aggregate_score_ge_2_count": aggregate,
        "safety_defaults": SAFETY_DEFAULTS,
        "safety_statements": SAFETY_STATEMENTS,
        "binding_recommendation": {
            "primary": "NetworkX remains the primary graph representation and correctness baseline.",
            "supplementary": [
                "igraph is adopted for algorithm-heavy read-only operations when benchmarks justify conversion.",
                "rustworkx is adopted when available for traversal/path hot spots with parity checks.",
            ],
            "deferred": [
                "graph-tool",
                "PyG",
                "DGL",
                "NetworkX-Temporal",
                "GraphScope except as a future GraphDB-selection candidate",
            ],
        },
    }


def render_m060c_s02_applicability_markdown(report: dict[str, Any]) -> str:
    """Render the matrix payload as Markdown."""
    lines = [
        "# M060c S02 Applicability Matrix",
        "",
        "This artifact compares 8 graph libraries across 5 future milestones. It is a decision aid only; it does not authorize graph writes or production imports.",
        "",
        "## Safety defaults",
        "",
    ]
    for key, value in report["safety_defaults"].items():
        lines.append(f"- `{key}={str(value).lower()}`")
    lines.extend(["", "Safety statements:", ""])
    lines.extend(f"- {statement}" for statement in report["safety_statements"])
    lines.extend(
        [
            "",
            f"Loopback host for any local-only checks: `{report['loopback_host']}`.",
            "",
            "## Aggregate score counts",
            "",
            "Count of milestone cells with `applicability_score >= 2`.",
            "",
            "| Library | Cells with score >= 2 | Decision posture |",
            "|---|---:|---|",
        ]
    )
    for library in report["libraries"]:
        count = report["aggregate_score_ge_2_count"][library]
        if library == "NetworkX":
            posture = "Primary baseline"
        elif library == "igraph":
            posture = "Adopt supplementary"
        elif library == "rustworkx":
            posture = "Adopt optional supplementary"
        elif library == "GraphScope":
            posture = "Defer except M063 evaluation"
        else:
            posture = "Defer / do not adopt now"
        lines.append(f"| {library} | {count} | {posture} |")

    lines.extend(
        [
            "",
            "## 8 libraries x 5 milestones matrix",
            "",
            "| Library | Milestone | Score | Use-case fit | Integration cost | Decision |",
            "|---|---|---:|---|---|---|",
        ]
    )
    for cell in report["cells"]:
        lines.append(
            "| {library} | {milestone} | {score} | {fit} | {cost} | {decision} |".format(
                library=cell["library"],
                milestone=cell["milestone"],
                score=cell["applicability_score"],
                fit=cell["use_case_fit"],
                cost=cell["integration_cost"],
                decision=cell["decision"],
            )
        )

    lines.extend(
        [
            "",
            "## Binding recommendation",
            "",
            "- NetworkX remains the primary graph representation and correctness baseline.",
            "- igraph is adopted as a supplementary read-only accelerator for algorithm-heavy operations in M060b and M061.",
            "- rustworkx is adopted as an optional supplementary read-only accelerator for traversal/path hot spots when available.",
            "- graph-tool, PyG, DGL, NetworkX-Temporal, and GraphScope are not authorized for runtime integration by this artifact.",
            "- GraphScope may be evaluated during M063 only as a GraphDB-selection candidate, not as a production write path.",
            "",
        ]
    )
    return "\n".join(lines)


def emit_m060c_s02_applicability_outputs(output_dir: Path = DEFAULT_OUTPUT_DIR) -> dict[str, Any]:
    """Write JSON and Markdown matrix artifacts and return the report."""
    output_dir.mkdir(parents=True, exist_ok=True)
    report = build_m060c_s02_applicability_matrix()
    json_path = output_dir / "applicability-matrix.json"
    markdown_path = output_dir / "applicability-matrix.md"
    report["metadata"] = {
        "json_path": str(json_path),
        "markdown_path": str(markdown_path),
        "source_benchmark": "artifacts/m060c-benchmark/benchmark.json",
        "research_dir": "artifacts/m060c-benchmark/library-research",
        "idempotent": True,
    }
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    markdown_path.write_text(render_m060c_s02_applicability_markdown(report), encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description="Emit M060c S02 applicability matrix artifacts.")
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    args = parser.parse_args()
    emit_m060c_s02_applicability_outputs(args.output_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
