"""Hybrid sidecar packet merge (M212) — ADR-008/009 composition without live services.

Pure application merge of GROBID + OpenDataLoader metric/evidence dicts into a
candidate hybrid packet. Does not call services, does not authorize import/writes.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal, Mapping

from research_graph.application.types import ResourceProfile
from research_graph.domain.universal_kb.contracts import SafetyFlags

HybridRouteCode = Literal[
    "grobid_header_plus_opendataloader_body",
    "grobid_fulltext_only",
    "opendataloader_only",
    "manual_review",
    "deferred_unavailable",
]

FieldOwner = Literal["grobid", "opendataloader", "none", "both"]

BODY_MARKDOWN_MIN_CHARS = 5000
HYBRID_RUNTIME_STAGE_NAME = "hybrid_sidecar_runtime"
HYBRID_RUNTIME_CONTRACT_VERSION = "hybrid-sidecar-runtime.v1"

HYBRID_RUNTIME_RESOURCE_PROFILE = ResourceProfile(
    llm_required=False,
    llm_provider=None,
    estimated_tokens=0,
    cpu_required=True,
    cpu_intensity="medium",
    io_required=True,
    io_type="network",
)


def _as_int(value: Any) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0


def _as_bool(value: Any) -> bool:
    return bool(value)


@dataclass(frozen=True, slots=True)
class HybridFieldOwnership:
    metadata: FieldOwner = "none"
    citations: FieldOwner = "none"
    body: FieldOwner = "none"
    layout: FieldOwner = "none"

    def to_dict(self) -> dict[str, str]:
        return {
            "metadata": self.metadata,
            "citations": self.citations,
            "body": self.body,
            "layout": self.layout,
        }


@dataclass(frozen=True, slots=True)
class HybridCandidatePacket:
    """Candidate-only hybrid parser packet (not import-eligible)."""

    paper_id: str
    route: HybridRouteCode
    ownership: HybridFieldOwnership
    body_markdown: str | None
    body_chars: int
    grobid_ok: bool
    odl_ok: bool
    odl_low_quality: bool
    confidence: Literal["high", "medium", "low"]
    diagnostics: tuple[str, ...] = ()
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    production_import_attempted: bool = False
    hybrid_claimed_success: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    resource_profile: ResourceProfile = field(
        default_factory=lambda: HYBRID_RUNTIME_RESOURCE_PROFILE
    )

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed or self.production_import_attempted:
            raise ValueError("hybrid packet cannot authorize import or writes")
        if self.hybrid_claimed_success and not (
            self.body_markdown and self.body_chars >= BODY_MARKDOWN_MIN_CHARS
        ):
            raise ValueError("hybrid_claimed_success requires body markdown evidence")

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "route": self.route,
            "ownership": self.ownership.to_dict(),
            "body_chars": self.body_chars,
            "has_body_markdown": bool(self.body_markdown),
            "grobid_ok": self.grobid_ok,
            "odl_ok": self.odl_ok,
            "odl_low_quality": self.odl_low_quality,
            "confidence": self.confidence,
            "diagnostics": list(self.diagnostics),
            "import_eligible": self.import_eligible,
            "graph_writes_allowed": self.graph_writes_allowed,
            "production_import_attempted": self.production_import_attempted,
            "hybrid_claimed_success": self.hybrid_claimed_success,
            "stage_name": HYBRID_RUNTIME_STAGE_NAME,
            "contract_version": HYBRID_RUNTIME_CONTRACT_VERSION,
            "resource_profile": {
                "llm_required": self.resource_profile.llm_required,
                "cpu_required": self.resource_profile.cpu_required,
                "cpu_intensity": self.resource_profile.cpu_intensity,
                "io_required": self.resource_profile.io_required,
                "io_type": self.resource_profile.io_type,
            },
            "safety_flags": self.safety_flags.to_dict(),
        }


def decide_hybrid_runtime_route(
    *,
    grobid_ok: bool,
    odl_ok: bool,
    odl_low_quality: bool,
    body_chars: int,
) -> tuple[HybridRouteCode, Literal["high", "medium", "low"], tuple[str, ...]]:
    """ADR-009-style route from availability + body quality (pure)."""
    diagnostics: list[str] = []
    body_ok = body_chars >= BODY_MARKDOWN_MIN_CHARS and not odl_low_quality

    if grobid_ok and odl_ok and body_ok:
        return (
            "grobid_header_plus_opendataloader_body",
            "high",
            ("route:hybrid", "body_ok", "adr009_default"),
        )
    if grobid_ok and (not odl_ok or odl_low_quality or not body_ok):
        if odl_low_quality:
            diagnostics.append("odl_low_quality")
        if not body_ok:
            diagnostics.append("body_below_threshold_or_missing")
        if not odl_ok:
            diagnostics.append("odl_unavailable")
        return (
            "grobid_fulltext_only",
            "medium",
            tuple(diagnostics) + ("route:grobid_fulltext_only", "adr009_fallback"),
        )
    if odl_ok and body_ok and not grobid_ok:
        return (
            "opendataloader_only",
            "medium",
            ("route:odl_only", "grobid_unavailable"),
        )
    if not grobid_ok and not odl_ok:
        return (
            "deferred_unavailable",
            "low",
            ("route:deferred", "both_sidecars_unavailable"),
        )
    return (
        "manual_review",
        "low",
        ("route:manual_review", f"body_chars:{body_chars}"),
    )


def merge_hybrid_sidecar_packets(
    *,
    paper_id: str,
    grobid: Mapping[str, Any] | None,
    opendataloader: Mapping[str, Any] | None,
    body_markdown: str | None = None,
) -> HybridCandidatePacket:
    """Merge metric dicts (+ optional body text) into a candidate hybrid packet."""
    g = dict(grobid or {})
    o = dict(opendataloader or {})

    grobid_status = str(g.get("status") or g.get("outcome") or "").lower()
    odl_status = str(o.get("status") or o.get("outcome") or "").lower()
    grobid_ok = bool(g) and grobid_status not in {"failed", "error", "unavailable", "blocked"}
    if g.get("error") and not g.get("header_title_present") and _as_int(g.get("bibl_count")) == 0:
        grobid_ok = False
    # Success signals from M055 packets
    if g.get("header_title_present") is True or _as_int(g.get("bibl_count")) > 0:
        grobid_ok = True
    if grobid_status in {"success", "ok", "completed"}:
        grobid_ok = True

    odl_low_quality = _as_bool(o.get("low_quality_source"))
    odl_ok = bool(o) and odl_status not in {"failed", "error", "unavailable", "blocked"}
    if _as_int(o.get("markdown_size_bytes")) > 0 or o.get("markdown"):
        odl_ok = True
    if odl_status in {"success", "ok", "completed"}:
        odl_ok = True

    md = body_markdown
    if md is None and isinstance(o.get("markdown"), str):
        md = o["markdown"]
    body_chars = len(md) if md else _as_int(o.get("markdown_size_bytes"))

    route, confidence, diag = decide_hybrid_runtime_route(
        grobid_ok=grobid_ok,
        odl_ok=odl_ok,
        odl_low_quality=odl_low_quality,
        body_chars=body_chars,
    )

    ownership = HybridFieldOwnership(
        metadata="grobid" if grobid_ok else "none",
        citations="grobid" if grobid_ok else "none",
        body=(
            "opendataloader"
            if odl_ok and body_chars >= BODY_MARKDOWN_MIN_CHARS and not odl_low_quality
            else ("grobid" if route == "grobid_fulltext_only" and body_chars > 0 else "none")
        ),
        layout=(
            "opendataloader"
            if odl_ok and _as_int(o.get("bounding_box_count")) > 0
            else ("grobid" if grobid_ok else "none")
        ),
    )

    hybrid_success = (
        route == "grobid_header_plus_opendataloader_body"
        and bool(md)
        and body_chars >= BODY_MARKDOWN_MIN_CHARS
        and not odl_low_quality
    )

    return HybridCandidatePacket(
        paper_id=paper_id,
        route=route,
        ownership=ownership,
        body_markdown=md if hybrid_success or (md and body_chars > 0) else None,
        body_chars=body_chars,
        grobid_ok=grobid_ok,
        odl_ok=odl_ok,
        odl_low_quality=odl_low_quality,
        confidence=confidence,
        diagnostics=diag
        + (
            f"grobid_ok:{grobid_ok}",
            f"odl_ok:{odl_ok}",
            f"body_chars:{body_chars}",
        ),
        hybrid_claimed_success=hybrid_success,
    )


__all__ = [
    "BODY_MARKDOWN_MIN_CHARS",
    "HYBRID_RUNTIME_CONTRACT_VERSION",
    "HYBRID_RUNTIME_RESOURCE_PROFILE",
    "HYBRID_RUNTIME_STAGE_NAME",
    "FieldOwner",
    "HybridCandidatePacket",
    "HybridFieldOwnership",
    "HybridRouteCode",
    "decide_hybrid_runtime_route",
    "merge_hybrid_sidecar_packets",
]
