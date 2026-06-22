#!/usr/bin/env python3
"""Verify the onion layering invariant (D086): the domain Core must not import
application or infrastructure packages.

The domain layer (``src/research_graph/domain/`` plus the pure typed-model
packages it depends on) is the innermost onion ring. Per D086 / doc/onion-layers.md,
it may import only the stdlib, ``research_graph.evaluation``,
``research_graph.papers``, and ``research_graph.domain.*``. Any import of an
application (``research_graph.application``) or infrastructure
(``research_graph.graph``/``corpus``/``llm``/``infrastructure``/...) package is a
layering violation.

This guard AST-scans the domain directory and fails (exit 1) on the first
reverse import, printing a JSON report. Exit 0 means the Core is clean.

Usage::

    uv run python scripts/verify_onion_layering.py
    uv run python scripts/verify_onion_layering.py --root src/research_graph/domain --json
"""

from __future__ import annotations

import argparse
import ast
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LAYER_ROOTS: dict[str, Path] = {
    "domain": ROOT / "src" / "research_graph" / "domain",
    "application": ROOT / "src" / "research_graph" / "application",
}

# Infrastructure + entry package prefixes that inner layers must NOT import.
# (Per doc/onion-layers.md.) Domain is the innermost ring; application is the
# next ring out and may import domain but NOT infrastructure.
INFRA_PREFIXES: tuple[str, ...] = (
    "research_graph.infrastructure",  # adapters
    "research_graph.graph",  # drivers
    "research_graph.corpus",  # sources / ingestion / parsing
    "research_graph.llm",  # LLM drivers
    "research_graph.retrieval",  # embedder / keyword extractor
    "research_graph.identity",
    "research_graph.quality",
    "research_graph.repair",
    "research_graph.staging",
    "research_graph.ops",
    "research_graph.workflows",  # entry / wiring
    "research_graph.cli",  # entry
)

# Per-layer forbidden import prefixes. Domain must not import application OR
# any infrastructure; application must not import any infrastructure
# (application may import domain + its own internals).
LAYER_FORBIDDEN: dict[str, tuple[str, ...]] = {
    "domain": ("research_graph.application", *INFRA_PREFIXES),
    "application": INFRA_PREFIXES,
}

DEFAULT_DOMAIN_ROOT = LAYER_ROOTS["domain"]
# Back-compat alias used by older call sites / docs.
FORBIDDEN_PREFIXES = LAYER_FORBIDDEN["domain"]

# Allowed domain-internal import roots (stdlib is always allowed).
ALLOWED_DOMAIN_ROOTS: tuple[str, ...] = (
    "research_graph.domain",  # the domain itself (schema/models/ports now live here, D086)
)


def _imported_modules(tree: ast.Module) -> list[str]:
    """Return absolute module names imported by ``tree`` (Import + ImportFrom)."""
    modules: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            modules.extend(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom):
            if node.module:
                modules.append(node.module)
    return modules


def _is_forbidden_for(module: str, forbidden: tuple[str, ...]) -> bool:
    return any(module == prefix or module.startswith(prefix + ".") for prefix in forbidden)


def _is_forbidden(module: str) -> bool:
    """Back-compat: domain forbidden set."""
    return _is_forbidden_for(module, FORBIDDEN_PREFIXES)


def scan_layer(layer: str, layer_root: Path) -> dict[str, object]:
    """Scan one onion layer root for forbidden imports. Returns a report dict."""
    forbidden = LAYER_FORBIDDEN[layer]
    violations: list[dict[str, object]] = []
    scanned: list[str] = []
    if not layer_root.exists():
        return {
            "layer": layer,
            "status": "error",
            "error": f"{layer} root not found: {layer_root}",
            "scanned_files": [],
            "violations": [],
        }
    for path in sorted(layer_root.rglob("*.py")):
        rel = str(path.relative_to(layer_root))
        scanned.append(rel)
        try:
            tree = ast.parse(path.read_text(encoding="utf-8"))
        except SyntaxError as exc:
            violations.append(
                {
                    "file": rel,
                    "module": "<syntax-error>",
                    "line": exc.lineno or 0,
                    "detail": str(exc),
                }
            )
            continue
        for module in _imported_modules(tree):
            if _is_forbidden_for(module, forbidden):
                violations.append(
                    {"file": rel, "module": module, "line": 0, "detail": "forbidden layer import"}
                )
    return {
        "layer": layer,
        "status": "clear" if not violations else "violations",
        "layer_root": str(layer_root),
        "scanned_files": scanned,
        "forbidden_prefixes": list(forbidden),
        "violation_count": len(violations),
        "violations": violations,
    }


def scan_domain(domain_root: Path) -> dict[str, object]:
    """Back-compat wrapper: scan the domain layer only."""
    return scan_layer("domain", domain_root)


def scan_all() -> dict[str, object]:
    """Scan every configured layer (domain, application). Returns an aggregate report."""
    reports = {layer: scan_layer(layer, root) for layer, root in LAYER_ROOTS.items()}
    total = sum(r["violation_count"] for r in reports.values())
    return {
        "status": "clear" if total == 0 else "violations",
        "violation_count": total,
        "layers": reports,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=None,
        help="scan a single directory (legacy domain-only mode); omit to scan all layers",
    )
    parser.add_argument(
        "--layer",
        choices=sorted(LAYER_ROOTS),
        default=None,
        help="scan one named layer (domain or application)",
    )
    parser.add_argument("--json", action="store_true", help="emit a JSON report to stdout")
    args = parser.parse_args()

    if args.root is not None:
        report = scan_layer("domain", args.root)
    elif args.layer is not None:
        report = scan_layer(args.layer, LAYER_ROOTS[args.layer])
    else:
        report = scan_all()

    if args.json:
        sys.stdout.write(json.dumps(report, indent=2) + "\n")
    elif "layers" in report:
        # Aggregate multi-layer report
        total = report["violation_count"]
        if total == 0:
            sys.stdout.write(
                f"onion layering guard ok: all layers clean "
                f"({', '.join(f'{name}={len(r["scanned_files"])} files' for name, r in report['layers'].items())}, "
                f"0 forbidden imports)\n"
            )
        else:
            sys.stderr.write(f"onion layering guard FAILED: {total} violation(s)\n")
            for layer, layer_report in report["layers"].items():
                for v in layer_report["violations"]:
                    sys.stderr.write(
                        f"  - [{layer}] {v['file']}: imports {v['module']} ({v['detail']})\n"
                    )
    else:
        status = report["status"]
        layer = report.get("layer", "domain")
        if status == "clear":
            sys.stdout.write(
                f"onion layering guard ok: {layer} clean "
                f"({len(report['scanned_files'])} files scanned, 0 forbidden imports)\n"
            )
        else:
            sys.stderr.write(
                f"onion layering guard FAILED: {report['violation_count']} violation(s)\n"
            )
            for v in report["violations"]:
                sys.stderr.write(f"  - {v['file']}: imports {v['module']} ({v['detail']})\n")
    return 0 if report["status"] == "clear" else 1


if __name__ == "__main__":
    raise SystemExit(main())
