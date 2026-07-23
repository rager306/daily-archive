"""Composition import-hold surface inventory (M236).

Read-only scan of workflows/composition Python modules for import_eligible
mentions and forbidden True enablements. Never authorizes import.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

_IMPORT_ELIGIBLE_MENTION = re.compile(r"\bimport_eligible\b")
_FORBIDDEN_ENABLE = re.compile(
    r"""["']?(?:import_eligible|graph_writes_allowed)["']?\s*[:=]\s*(?:True|true)\b"""
)


def inventory_composition_import_hold(composition_dir: Path) -> dict[str, Any]:
    """Inventory composition modules for import-hold surface and enablements.

    Returns a fail-closed report. ``enablement_hits`` empty means no True
    assignments for import_eligible / graph_writes_allowed were found.
    """
    root = Path(composition_dir)
    files = sorted(p for p in root.glob("*.py") if p.is_file())
    modules: list[str] = []
    hits: list[str] = []

    for path in files:
        text = path.read_text(encoding="utf-8")
        if _IMPORT_ELIGIBLE_MENTION.search(text):
            modules.append(path.name)
        for i, line in enumerate(text.splitlines(), start=1):
            # Skip comments-only lines to reduce false positives.
            stripped = line.lstrip()
            if stripped.startswith("#"):
                continue
            if _FORBIDDEN_ENABLE.search(line):
                hits.append(f"{path.name}:{i}:{stripped[:120]}")

    return {
        "schema_version": "m236-composition-import-hold-inventory.v1",
        "composition_dir": str(root),
        "scanned_file_count": len(files),
        "module_count": len(modules),
        "modules_with_import_eligible": modules,
        "enablement_hit_count": len(hits),
        "enablement_hits": hits,
        "import_eligible": False,
        "graph_writes_allowed": False,
    }


__all__ = ["inventory_composition_import_hold"]
