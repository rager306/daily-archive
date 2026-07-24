"""Import-hold surface inventory (M236/M237/M238).

Read-only scan of Python modules for ``import_eligible`` mentions and forbidden
Python True enablements of ``import_eligible`` / ``graph_writes_allowed``.

M236: composition-only helper.
M237: multi-tree scan + precise assignment pattern (avoids docstring/string
markers like ``import_eligible: true``).
M238: default package roots (domain/application/composition/infrastructure).

Never authorizes import.
"""

from __future__ import annotations

import re
from collections.abc import Sequence
from pathlib import Path
from typing import Any

_IMPORT_ELIGIBLE_MENTION = re.compile(r"\bimport_eligible\b")
# Python True assignment / typed default only (not JSON-ish ": true" markers).
_FORBIDDEN_ENABLE = re.compile(
    r"""\b(?:import_eligible|graph_writes_allowed)\b\s*=\s*True\b"""
)


def default_import_hold_roots() -> list[Path]:
    """Package-relative roots for the import-hold default inventory.

    Order: domain → application → composition → infrastructure.
    Paths resolve from the installed/source ``research_graph`` package, not cwd.
    """
    # .../research_graph/application/corpus/this_file.py → research_graph/
    pkg_root = Path(__file__).resolve().parents[2]
    return [
        pkg_root / "domain",
        pkg_root / "application",
        pkg_root / "workflows" / "composition",
        pkg_root / "infrastructure",
    ]


def _scan_tree(root: Path) -> tuple[list[str], list[str], int]:
    """Return (modules_with_mention, enablement_hits, scanned_file_count)."""
    if not root.is_dir():
        return [], [], 0
    files = sorted(p for p in root.rglob("*.py") if p.is_file())
    modules: list[str] = []
    hits: list[str] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        rel = path.name if root == path.parent else str(path.relative_to(root))
        if _IMPORT_ELIGIBLE_MENTION.search(text):
            modules.append(rel.replace("\\", "/"))
        for i, line in enumerate(text.splitlines(), start=1):
            stripped = line.lstrip()
            if stripped.startswith("#"):
                continue
            if _FORBIDDEN_ENABLE.search(line):
                hits.append(f"{rel}:{i}:{stripped[:120]}")
    return modules, hits, len(files)


def inventory_import_hold_trees(roots: Sequence[Path]) -> dict[str, Any]:
    """Inventory one or more source trees for import-hold surfaces.

    Fail-closed report: never sets import_eligible/graph_writes_allowed true.
    """
    all_modules: list[str] = []
    all_hits: list[str] = []
    scanned = 0
    root_labels: list[str] = []
    for root in roots:
        path = Path(root)
        root_labels.append(str(path))
        modules, hits, count = _scan_tree(path)
        # Prefix modules/hits with root basename when multi-tree for uniqueness.
        label = path.name
        for m in modules:
            all_modules.append(f"{label}/{m}")
        for h in hits:
            all_hits.append(f"{label}/{h}")
        scanned += count

    return {
        "schema_version": "import-hold-inventory.v1",
        "roots": root_labels,
        "tree_count": len(root_labels),
        "scanned_file_count": scanned,
        "module_count": len(all_modules),
        "modules_with_import_eligible": sorted(all_modules),
        "enablement_hit_count": len(all_hits),
        "enablement_hits": all_hits,
        "import_eligible": False,
        "graph_writes_allowed": False,
    }


def inventory_composition_import_hold(composition_dir: Path) -> dict[str, Any]:
    """Inventory composition modules (M236 API; flat top-level *.py only)."""
    root = Path(composition_dir)
    files = sorted(p for p in root.glob("*.py") if p.is_file())
    modules: list[str] = []
    hits: list[str] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        if _IMPORT_ELIGIBLE_MENTION.search(text):
            modules.append(path.name)
        for i, line in enumerate(text.splitlines(), start=1):
            stripped = line.lstrip()
            if stripped.startswith("#"):
                continue
            if _FORBIDDEN_ENABLE.search(line):
                hits.append(f"{path.name}:{i}:{stripped[:120]}")
    return {
        "schema_version": "composition-import-hold-inventory.v1",
        "composition_dir": str(root),
        "scanned_file_count": len(files),
        "module_count": len(modules),
        "modules_with_import_eligible": modules,
        "enablement_hit_count": len(hits),
        "enablement_hits": hits,
        "import_eligible": False,
        "graph_writes_allowed": False,
    }


__all__ = [
    "default_import_hold_roots",
    "inventory_composition_import_hold",
    "inventory_import_hold_trees",
]
