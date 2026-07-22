"""Parser body-route policy for graph-prep composition (M211).

Pure application policy: decides which body source is authoritative for
structure/readiness without executing hybrid services or claiming hybrid success.
Queue/scheduler remain seams only (ADR-017 / ADR-027).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.application.types import ResourceProfile
from research_graph.domain.universal_kb.contracts import SafetyFlags

BodyRoute = Literal[
    "html_native",
    "mdconverter",
    "fitz_offline",
    "hybrid_deferred",
    "unavailable",
]

BodyPreference = Literal["auto", "html", "mdconverter", "fitz", "hybrid"]

# Stage names reserved for future UniversalKBQueue enqueue (D085) — not activated.
BODY_RESOLVE_STAGE_NAME = "parser_body_resolve"
BODY_RESOLVE_CONTRACT_VERSION = "parser-body-resolve.v1"

BODY_RESOLVE_RESOURCE_PROFILE = ResourceProfile(
    llm_required=False,
    llm_provider=None,
    estimated_tokens=0,
    cpu_required=False,
    cpu_intensity="light",
    io_required=True,
    io_type="network",
)


@dataclass(frozen=True, slots=True)
class BodyRouteDecision:
    """Pure policy decision before any I/O."""

    route: BodyRoute
    reason: str
    hybrid_available: bool = False
    hybrid_claimed_success: bool = False
    diagnostics: tuple[str, ...] = ()
    resource_profile: ResourceProfile = field(default_factory=lambda: BODY_RESOLVE_RESOURCE_PROFILE)
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.hybrid_claimed_success and self.route != "hybrid_deferred":
            # Only a future hybrid runtime may claim success; policy never does.
            raise ValueError("body route policy cannot claim hybrid success")
        if self.hybrid_claimed_success:
            raise ValueError("M211 policy forbids hybrid_claimed_success=true")

    def to_dict(self) -> dict[str, Any]:
        return {
            "route": self.route,
            "reason": self.reason,
            "hybrid_available": self.hybrid_available,
            "hybrid_claimed_success": self.hybrid_claimed_success,
            "diagnostics": list(self.diagnostics),
            "resource_profile": {
                "llm_required": self.resource_profile.llm_required,
                "cpu_required": self.resource_profile.cpu_required,
                "cpu_intensity": self.resource_profile.cpu_intensity,
                "io_required": self.resource_profile.io_required,
                "io_type": self.resource_profile.io_type,
            },
            "stage_name": BODY_RESOLVE_STAGE_NAME,
            "contract_version": BODY_RESOLVE_CONTRACT_VERSION,
            "safety_flags": self.safety_flags.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class BodyRouteIntent:
    """Inputs for pure routing (no filesystem, no network)."""

    preference: BodyPreference = "auto"
    has_local_html: bool = False
    has_local_markdown: bool = False
    has_local_pdf: bool = False
    has_arxiv_id: bool = False
    fulltext_provider_available: bool = False
    fitz_fallback_allowed: bool = True
    hybrid_runtime_available: bool = False


def decide_body_route(intent: BodyRouteIntent) -> BodyRouteDecision:
    """Decide body route without I/O. Never reports hybrid success."""
    # Explicit preference short-circuits when feasible.
    if intent.preference == "html":
        if intent.has_local_html or intent.has_arxiv_id:
            return BodyRouteDecision(
                route="html_native",
                reason="preference_html",
                diagnostics=("prefer:html",),
            )
        return BodyRouteDecision(
            route="unavailable",
            reason="html_preferred_but_no_html_source",
            diagnostics=("prefer:html", "missing_html_source"),
        )

    if intent.preference == "mdconverter":
        if intent.fulltext_provider_available and (intent.has_arxiv_id or intent.has_local_pdf):
            return BodyRouteDecision(
                route="mdconverter",
                reason="preference_mdconverter",
                diagnostics=("prefer:mdconverter", "fulltext_provider_available"),
            )
        return BodyRouteDecision(
            route="unavailable",
            reason="mdconverter_preferred_but_unavailable",
            diagnostics=("prefer:mdconverter", "provider_or_source_missing"),
        )

    if intent.preference == "fitz":
        if intent.fitz_fallback_allowed and intent.has_local_pdf:
            return BodyRouteDecision(
                route="fitz_offline",
                reason="preference_fitz",
                diagnostics=("prefer:fitz", "offline_pdf_text"),
            )
        return BodyRouteDecision(
            route="unavailable",
            reason="fitz_preferred_but_no_pdf",
            diagnostics=("prefer:fitz", "missing_pdf"),
        )

    if intent.preference == "hybrid":
        # Honest: hybrid runtime not composed in M211.
        return BodyRouteDecision(
            route="hybrid_deferred",
            reason="hybrid_requested_but_runtime_not_composed",
            hybrid_available=intent.hybrid_runtime_available,
            diagnostics=(
                "prefer:hybrid",
                "adr008_adr009_binding",
                "runtime_not_wired_m211",
                "do_not_claim_hybrid_success",
            ),
        )

    # auto policy: prefer already-local text, then html, then mdconverter, then fitz.
    if intent.has_local_markdown:
        return BodyRouteDecision(
            route="html_native",
            reason="auto_local_markdown_as_text_body",
            diagnostics=("auto", "local_markdown"),
        )
    if intent.has_local_html:
        return BodyRouteDecision(
            route="html_native",
            reason="auto_local_html",
            diagnostics=("auto", "local_html"),
        )
    if intent.has_arxiv_id and intent.has_local_html is False:
        # Remote/HTML-capable id: prefer native HTML acquire path first (cheap, wired).
        return BodyRouteDecision(
            route="html_native",
            reason="auto_arxiv_html_first",
            diagnostics=("auto", "arxiv_html_preferred_over_pdf_stack"),
        )
    if intent.fulltext_provider_available and (intent.has_arxiv_id or intent.has_local_pdf):
        return BodyRouteDecision(
            route="mdconverter",
            reason="auto_mdconverter",
            diagnostics=("auto", "fulltext_provider"),
        )
    if intent.fitz_fallback_allowed and intent.has_local_pdf:
        return BodyRouteDecision(
            route="fitz_offline",
            reason="auto_fitz_last_resort",
            diagnostics=("auto", "fitz_offline_fallback", "not_hybrid"),
        )
    if intent.hybrid_runtime_available:
        return BodyRouteDecision(
            route="hybrid_deferred",
            reason="auto_hybrid_runtime_flag_but_not_composed",
            hybrid_available=True,
            diagnostics=("auto", "hybrid_flag_true_but_m211_defers"),
        )
    return BodyRouteDecision(
        route="unavailable",
        reason="auto_no_body_source",
        diagnostics=("auto", "no_html_md_pdf_or_provider"),
    )


__all__ = [
    "BODY_RESOLVE_CONTRACT_VERSION",
    "BODY_RESOLVE_RESOURCE_PROFILE",
    "BODY_RESOLVE_STAGE_NAME",
    "BodyPreference",
    "BodyRoute",
    "BodyRouteDecision",
    "BodyRouteIntent",
    "decide_body_route",
]
