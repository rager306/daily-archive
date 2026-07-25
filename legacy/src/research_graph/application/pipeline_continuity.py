"""Post-M208 pipeline continuity audit (M209).

Typed seven-layer continuity checklist for graph-data readiness work.
Does not run acquisition, Falkor, or production import. Fail-closed flags stay false.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from research_graph.domain.universal_kb.contracts import SafetyFlags

LayerName = Literal[
    "source",
    "parser",
    "structure",
    "extraction",
    "graph",
    "review",
    "agents",
]
LayerHealth = Literal["present", "partial", "gap", "blocked"]

PIPELINE_LAYERS: tuple[LayerName, ...] = (
    "source",
    "parser",
    "structure",
    "extraction",
    "graph",
    "review",
    "agents",
)

# Expected seam paths relative to repo root (existence-only inventory).
DEFAULT_LAYER_SEAMS: dict[LayerName, tuple[str, ...]] = {
    "source": (
        "src/research_graph/infrastructure/corpus/ingestion/loader.py",
        "src/research_graph/workflows/composition/universal_source.py",
        "src/research_graph/application/analyze_source.py",
        "src/research_graph/application/corpus/catalog_coverage_reconciliation.py",
    ),
    "parser": (
        "src/research_graph/infrastructure/corpus/parsing/parser.py",
        "src/research_graph/application/corpus/parser_replay.py",
    ),
    "structure": (
        "src/research_graph/infrastructure/papers/indexing/parsed_page_index.py",
        "src/research_graph/infrastructure/papers/semantic_chunks.py",
    ),
    "extraction": (
        "src/research_graph/application/paper_extraction.py",
        "src/research_graph/application/chunk_extraction.py",
        "src/research_graph/application/extraction_pilot.py",
        "src/research_graph/application/reviewed_extraction_metrics.py",
    ),
    "graph": (
        "src/research_graph/domain/ports.py",
        "src/research_graph/infrastructure/graph/projection_backends.py",
        "src/research_graph/infrastructure/graph/graph_read_adapters.py",
        "src/research_graph/application/graph/falkor_operation_plan.py",
        "src/research_graph/workflows/composition/graph_data_readiness.py",
    ),
    "review": (
        "src/research_graph/application/graph/promotion_boundary.py",
        "src/research_graph/infrastructure/staging/import_boundary.py",
        "src/research_graph/infrastructure/graph/readiness/review.py",
        "src/research_graph/application/corpus/catalog_coverage_reconciliation.py",
    ),
    "agents": (
        "src/research_graph/workflows/composition/symfsm_operators.py",
        "src/research_graph/workflows/composition/symfsm_loop.py",
        "src/research_graph/workflows/rlm/graph_traversal.py",
    ),
}

# Known post-M208 gaps (not missing modules — missing production wiring).
DEFAULT_KNOWN_GAPS: dict[LayerName, tuple[str, ...]] = {
    "source": ("pdf_body_not_fulltext", "no_batch_filter_in_core_loader"),
    "parser": ("real_pdf_quality_variance",),
    "structure": ("real_corpus_chunk_quality_not_continuously_gated",),
    "extraction": ("live_llm_optional_not_fleet",),
    "graph": (
        "no_live_falkor_driver_by_policy",
        "composition_root_missing_pre_m209",
        "cli_not_wired_to_projection",
    ),
    "review": ("operator_cli_not_wired", "pilot_eligible_not_import_eligible"),
    "agents": ("symfsm_not_cli_wired", "experience_store_deferred"),
}


@dataclass(frozen=True, slots=True)
class LayerStatus:
    layer: LayerName
    health: LayerHealth
    present_seams: tuple[str, ...]
    missing_seams: tuple[str, ...]
    gaps: tuple[str, ...]
    notes: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "layer": self.layer,
            "health": self.health,
            "present_seams": list(self.present_seams),
            "missing_seams": list(self.missing_seams),
            "gaps": list(self.gaps),
            "notes": self.notes,
        }


@dataclass(frozen=True, slots=True)
class ContinuityAudit:
    """Seven-layer continuity inventory with fail-closed graph flags."""

    layers: tuple[LayerStatus, ...]
    overall: LayerHealth
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    production_import_attempted: bool = False
    falkor_touched: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed or self.production_import_attempted:
            raise ValueError("continuity audit cannot authorize import or writes")
        if self.falkor_touched:
            raise ValueError("M209 continuity audit must not touch FalkorDB")
        if len(self.layers) != 7:
            raise ValueError("continuity audit requires exactly 7 layers")

    def to_dict(self) -> dict[str, Any]:
        return {
            "layers": [layer.to_dict() for layer in self.layers],
            "overall": self.overall,
            "import_eligible": self.import_eligible,
            "graph_writes_allowed": self.graph_writes_allowed,
            "production_import_attempted": self.production_import_attempted,
            "falkor_touched": self.falkor_touched,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }

    def gap_codes(self) -> tuple[str, ...]:
        codes: list[str] = []
        for layer in self.layers:
            codes.extend(f"{layer.layer}:{gap}" for gap in layer.gaps)
            codes.extend(f"{layer.layer}:missing:{path}" for path in layer.missing_seams)
        return tuple(codes)


def _layer_status(
    layer: LayerName,
    *,
    repo_root: Path,
    seams: tuple[str, ...],
    known_gaps: tuple[str, ...],
) -> LayerStatus:
    present: list[str] = []
    missing: list[str] = []
    for rel in seams:
        path = repo_root / rel
        if path.exists():
            present.append(rel)
        else:
            missing.append(rel)
    if missing and not present:
        health: LayerHealth = "blocked"
    elif missing or known_gaps:
        health = "partial" if present else "gap"
    else:
        health = "present"
    # Known production-wiring gaps keep layer at most partial even if files exist.
    if known_gaps and health == "present":
        health = "partial"
    return LayerStatus(
        layer=layer,
        health=health,
        present_seams=tuple(present),
        missing_seams=tuple(missing),
        gaps=known_gaps,
        notes=f"seams:{len(present)}/{len(seams)}",
    )


def build_continuity_audit(
    *,
    repo_root: str | Path = ".",
    layer_seams: dict[LayerName, tuple[str, ...]] | None = None,
    known_gaps: dict[LayerName, tuple[str, ...]] | None = None,
) -> ContinuityAudit:
    """Build existence-only continuity inventory for the 7-layer pipeline."""
    root = Path(repo_root)
    seams_map = layer_seams or DEFAULT_LAYER_SEAMS
    gaps_map = known_gaps or DEFAULT_KNOWN_GAPS
    layers = tuple(
        _layer_status(
            layer,
            repo_root=root,
            seams=seams_map.get(layer, ()),
            known_gaps=gaps_map.get(layer, ()),
        )
        for layer in PIPELINE_LAYERS
    )
    healths = {layer.health for layer in layers}
    if "blocked" in healths:
        overall: LayerHealth = "blocked"
    elif "gap" in healths:
        overall = "gap"
    elif "partial" in healths:
        overall = "partial"
    else:
        overall = "present"
    return ContinuityAudit(
        layers=layers,
        overall=overall,
        diagnostics=(
            "post_agent_loop_continuity_inventory",
            "falkor_deferred_by_policy",
            "import_write_fail_closed",
            f"overall:{overall}",
        ),
    )


def render_continuity_report(audit: ContinuityAudit) -> str:
    """Render a compact markdown continuity report (metadata only)."""
    lines = [
        "# Pipeline Continuity Audit",
        "",
        f"- overall: `{audit.overall}`",
        f"- import_eligible: `{audit.import_eligible}`",
        f"- graph_writes_allowed: `{audit.graph_writes_allowed}`",
        f"- falkor_touched: `{audit.falkor_touched}`",
        "",
        "| Layer | Health | Present | Missing | Gaps |",
        "|-------|--------|---------|---------|------|",
    ]
    for layer in audit.layers:
        lines.append(
            f"| {layer.layer} | {layer.health} | {len(layer.present_seams)} | "
            f"{len(layer.missing_seams)} | {len(layer.gaps)} |"
        )
    lines.extend(["", "## Gap codes", ""])
    for code in audit.gap_codes():
        lines.append(f"- `{code}`")
    lines.append("")
    return "\n".join(lines)


__all__ = [
    "DEFAULT_KNOWN_GAPS",
    "DEFAULT_LAYER_SEAMS",
    "ContinuityAudit",
    "LayerHealth",
    "LayerName",
    "LayerStatus",
    "PIPELINE_LAYERS",
    "build_continuity_audit",
    "render_continuity_report",
]
