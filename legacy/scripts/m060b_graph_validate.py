#!/usr/bin/env python3
"""Validate the M058 four-layer graph manifest with NetworkX.

The validator is read-only and diagnostic-only. It validates artifact-level graph
structure, safety defaults, a five-PDF content hash sample, and paper-level layer
separation warnings for the M058 combined manifest.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

import networkx as nx

# pyrefly: ignore [missing-import]
from m060b_graph_stats import (
    DEFAULT_MANIFEST,
    DEFAULT_OUTPUT_DIR,
    LOOPBACK_BIND_HOST,
    ROOT,
    SAFETY_DEFAULTS,
    build_graph,
    edge_layer,
    edge_source,
    edge_target,
    read_json,
)

DEFAULT_PDF_MANIFEST = ROOT / "artifacts" / "m054-pdf-acquisition" / "manifest.json"


def check_result(check_id: str, status: str, message: str, details: Any = None) -> dict[str, Any]:
    """Build one validation check result."""
    return {"id": check_id, "status": status, "message": message, "details": details}


def validate_safety_defaults(payload: dict[str, Any]) -> dict[str, Any]:
    """Validate that all five safety defaults are explicit and false."""
    actual = payload.get("safety_defaults")
    if actual == SAFETY_DEFAULTS:
        return check_result(
            "safety_defaults_explicit_false",
            "PASS",
            "All five safety defaults are explicit and false.",
            actual,
        )
    return check_result(
        "safety_defaults_explicit_false",
        "FAIL",
        "Safety defaults are missing or not all false.",
        {"expected": SAFETY_DEFAULTS, "actual": actual},
    )


def validate_loopback(payload: dict[str, Any]) -> dict[str, Any]:
    """Validate the manifest loopback host contract."""
    actual = payload.get("loopback_bind_host")
    if actual == LOOPBACK_BIND_HOST:
        return check_result(
            "loopback_bind_host", "PASS", "Loopback bind host is 127.0.0.1.", actual
        )
    return check_result(
        "loopback_bind_host",
        "FAIL",
        "Loopback bind host is not the required 127.0.0.1 value.",
        actual,
    )


def validate_citation_orphans(edges: list[dict[str, Any]]) -> dict[str, Any]:
    """Validate that citation-layer nodes have no zero-degree nodes."""
    citation_edges = [edge for edge in edges if edge.get("evidence_layer") == "citation"]
    graph = build_graph(citation_edges)
    orphans = sorted(str(node) for node, degree in graph.degree() if degree == 0)
    if not orphans:
        return check_result(
            "citation_orphans",
            "PASS",
            "No orphan nodes were found in the citation layer.",
            {"checked_nodes": graph.number_of_nodes()},
        )
    return check_result(
        "citation_orphans",
        "FAIL",
        "Citation-layer orphan nodes were found.",
        {"count": len(orphans), "nodes": orphans[:50]},
    )


def validate_duplicate_edges(edges: list[dict[str, Any]]) -> dict[str, Any]:
    """Validate there are no duplicate artifact edges within one layer."""
    duplicate_counter = Counter(
        (edge_layer(edge), edge_source(edge), edge_target(edge)) for edge in edges
    )
    duplicates = [
        {"layer": layer, "source": source, "target": target, "count": count}
        for (layer, source, target), count in sorted(duplicate_counter.items())
        if count > 1
    ]
    if not duplicates:
        return check_result(
            "duplicate_edges_per_layer",
            "PASS",
            "No duplicate artifact edges were found within any layer.",
            {"checked_edges": len(edges)},
        )
    return check_result(
        "duplicate_edges_per_layer",
        "FAIL",
        "Duplicate artifact edges were found within at least one layer.",
        {"count": len(duplicates), "duplicates": duplicates[:50]},
    )


def validate_self_loops(edges: list[dict[str, Any]]) -> dict[str, Any]:
    """Validate the artifact-level NetworkX graph has no self-loops."""
    graph = build_graph(edges)
    self_loops = [
        {"source": str(source), "target": str(target)}
        for source, target in nx.selfloop_edges(graph)
    ]
    if not self_loops:
        return check_result(
            "self_loops",
            "PASS",
            "No artifact-level self-loops were found.",
            {"checked_edges": len(edges)},
        )
    return check_result(
        "self_loops",
        "FAIL",
        "Artifact-level self-loops were found.",
        {"count": len(self_loops), "self_loops": self_loops[:50]},
    )


def resolve_pdf_path(pdf_entry: dict[str, Any]) -> Path | None:
    """Resolve the best local PDF path from a PDF manifest entry."""
    for key in ("path", "pdf_path", "source_pdf_path", "local_path"):
        value = pdf_entry.get(key)
        if isinstance(value, str) and value:
            path = Path(value)
            return path if path.is_absolute() else ROOT / path
    return None


def validate_content_sha256(pdf_manifest_path: Path, sample_size: int = 5) -> dict[str, Any]:
    """Validate content_sha256 for the first N PDFs in the sample manifest."""
    payload = read_json(pdf_manifest_path)
    pdfs = payload.get("pdfs")
    if not isinstance(pdfs, list):
        return check_result(
            "content_sha256_sample",
            "FAIL",
            "PDF manifest does not contain a pdfs array.",
            {"manifest": str(pdf_manifest_path)},
        )

    checked: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for pdf in pdfs[:sample_size]:
        if not isinstance(pdf, dict):
            failures.append({"error": "PDF entry is not an object"})
            continue
        path = resolve_pdf_path(pdf)
        expected = pdf.get("content_sha256")
        article_key = pdf.get("article_key")
        if path is None or not isinstance(expected, str) or not expected:
            failures.append({"article_key": article_key, "error": "Missing path or content_sha256"})
            continue
        if not path.exists():
            failures.append(
                {"article_key": article_key, "path": str(path), "error": "PDF file missing"}
            )
            continue
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        result = {
            "article_key": article_key,
            "path": str(path.relative_to(ROOT)),
            "matches": actual == expected,
        }
        checked.append(result)
        if actual != expected:
            failures.append(
                {
                    "article_key": article_key,
                    "path": str(path),
                    "expected": expected,
                    "actual": actual,
                }
            )

    if not failures and len(checked) == sample_size:
        return check_result(
            "content_sha256_sample",
            "PASS",
            f"content_sha256 matches actual file bytes for {sample_size} sampled PDFs.",
            {"checked": checked},
        )
    return check_result(
        "content_sha256_sample",
        "FAIL",
        "content_sha256 sample validation failed.",
        {"checked": checked, "failures": failures},
    )


def validate_layer_separation(edges: list[dict[str, Any]]) -> dict[str, Any]:
    """Flag paper pairs that appear in more than one evidence layer."""
    layers_by_pair: dict[tuple[str, str], set[str]] = defaultdict(set)
    for edge in edges:
        source = edge.get("source_paper_id")
        target = edge.get("target_paper_id")
        if isinstance(source, str) and isinstance(target, str):
            layers_by_pair[(source, target)].add(edge_layer(edge))

    repeated = [
        {"source_paper_id": source, "target_paper_id": target, "layers": sorted(layers)}
        for (source, target), layers in sorted(layers_by_pair.items())
        if len(layers) > 1
    ]
    if not repeated:
        return check_result(
            "paper_pair_layer_separation",
            "PASS",
            "No paper pair appears in more than one layer.",
            {"flagged_pairs": 0},
        )
    return check_result(
        "paper_pair_layer_separation",
        "WARN",
        "Some paper pairs appear in multiple layers; this is allowed but flagged.",
        {"flagged_pairs": len(repeated), "pairs": repeated[:50]},
    )


def validate_manifest(
    manifest_path: Path, pdf_manifest_path: Path = DEFAULT_PDF_MANIFEST
) -> dict[str, Any]:
    """Run all graph-manifest validation checks."""
    payload = read_json(manifest_path)
    edges = payload.get("edges")
    if not isinstance(edges, list):
        raise ValueError(f"Manifest {manifest_path} does not contain an edges array")

    checks = [
        validate_safety_defaults(payload),
        validate_loopback(payload),
        validate_citation_orphans(edges),
        validate_duplicate_edges(edges),
        validate_self_loops(edges),
        validate_content_sha256(pdf_manifest_path),
        validate_layer_separation(edges),
    ]
    failed = [check for check in checks if check["status"] == "FAIL"]
    warned = [check for check in checks if check["status"] == "WARN"]
    overall_status = "fail" if failed else "pass_with_warnings" if warned else "pass"
    return {
        "manifest_path": str(manifest_path),
        "pdf_manifest_path": str(pdf_manifest_path),
        "overall_status": overall_status,
        "summary": {
            "passed": len([c for c in checks if c["status"] == "PASS"]),
            "warnings": len(warned),
            "failed": len(failed),
        },
        "checks": checks,
    }


def resolve_output_paths(output: Path | None) -> tuple[Path, Path]:
    """Resolve JSON and Markdown output paths from an optional CLI target."""
    if output is None:
        output_dir = DEFAULT_OUTPUT_DIR
        return output_dir / "validation.json", output_dir / "validation.md"
    if output.suffix == ".json":
        return output, output.with_suffix(".md")
    return output / "validation.json", output / "validation.md"


def write_validation(validation: dict[str, Any], json_path: Path, md_path: Path) -> None:
    """Write JSON and Markdown validation reports idempotently."""
    json_path.parent.mkdir(parents=True, exist_ok=True)
    md_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(validation, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_path.write_text(render_markdown(validation), encoding="utf-8")


def render_markdown(validation: dict[str, Any]) -> str:
    """Render validation results as Markdown."""
    lines = [
        "# M060b NetworkX Graph Validation",
        "",
        "This report is read-only. Production import is not authorized. Graph writes are disabled.",
        "External network access is disabled. LLM calls are disabled. Fact promotion is disabled.",
        "",
        f"Overall status: `{validation['overall_status']}`",
        "",
        "## Summary",
        "",
        f"- Passed checks: {validation['summary']['passed']}",
        f"- Warnings: {validation['summary']['warnings']}",
        f"- Failed checks: {validation['summary']['failed']}",
        "",
        "## Checks",
        "",
        "| Check | Status | Message |",
        "|---|---|---|",
    ]
    for check in validation["checks"]:
        lines.append(f"| `{check['id']}` | `{check['status']}` | {check['message']} |")

    lines.extend(["", "## Flagged Layer Separation", ""])
    layer_check = next(
        (check for check in validation["checks"] if check["id"] == "paper_pair_layer_separation"),
        None,
    )
    pairs = [] if not layer_check else layer_check.get("details", {}).get("pairs", [])
    if pairs:
        for pair in pairs[:10]:
            lines.append(
                "- `{source}` -> `{target}` appears in layers: {layers}".format(
                    source=pair["source_paper_id"],
                    target=pair["target_paper_id"],
                    layers=", ".join(pair["layers"]),
                )
            )
    else:
        lines.append("- None.")
    return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest", type=Path, default=DEFAULT_MANIFEST, help="Graph manifest JSON path"
    )
    parser.add_argument(
        "--pdf-manifest",
        type=Path,
        default=DEFAULT_PDF_MANIFEST,
        help="PDF manifest used for the five-file content_sha256 sample",
    )
    parser.add_argument(
        "--output", type=Path, default=None, help="Output directory or validation JSON path"
    )
    return parser.parse_args()


def display_path(path: Path) -> str:
    """Return a stable display path for repository-relative or absolute outputs."""
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def main() -> int:
    args = parse_args()
    manifest_path = args.manifest if args.manifest.is_absolute() else ROOT / args.manifest
    pdf_manifest_path = (
        args.pdf_manifest if args.pdf_manifest.is_absolute() else ROOT / args.pdf_manifest
    )
    validation = validate_manifest(manifest_path, pdf_manifest_path)
    json_path, md_path = resolve_output_paths(args.output)
    if not json_path.is_absolute():
        json_path = ROOT / json_path
    if not md_path.is_absolute():
        md_path = ROOT / md_path
    write_validation(validation, json_path, md_path)
    sys.stdout.write(f"Wrote {display_path(json_path)}\n")
    sys.stdout.write(f"Wrote {display_path(md_path)}\n")
    return 1 if validation["overall_status"] == "fail" else 0


if __name__ == "__main__":
    raise SystemExit(main())
