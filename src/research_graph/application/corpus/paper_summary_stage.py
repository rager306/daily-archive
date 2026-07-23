"""Optional paper summary stage package (M254 S04).

Pure application package for an optional ETL summary stage.
Never authorizes import or graph writes. Composition must keep the
stage off by default.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from research_graph.domain.universal_kb.contracts import SafetyFlags

SCHEMA_VERSION = "m254-paper-summary-stage.v1"


@dataclass(frozen=True, slots=True)
class PaperSummaryStagePackage:
    schema_version: str
    paper_id: str
    title: str
    abstract: str
    headline: str
    what_it_does: str
    why_it_matters: str
    analogy: str
    binding_id: str
    model_name: str
    role: str
    stage_status: str  # pending | ready_for_review | failed
    safety_flags: SafetyFlags
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    error: str | None = None

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("paper summary stage cannot authorize import/writes")
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "paper_id": self.paper_id,
            "title": self.title,
            "abstract": self.abstract,
            "headline": self.headline,
            "what_it_does": self.what_it_does,
            "why_it_matters": self.why_it_matters,
            "analogy": self.analogy,
            "binding_id": self.binding_id,
            "model_name": self.model_name,
            "role": self.role,
            "stage_status": self.stage_status,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "error": self.error,
            "note": (
                "Optional summary stage only; never import authority; "
                "composition default off"
            ),
        }


def build_paper_summary_stage(
    *,
    paper_id: str,
    title: str,
    abstract: str,
    headline: str,
    what_it_does: str,
    why_it_matters: str,
    analogy: str,
    binding_id: str,
    model_name: str,
    role: str = "default",
    error: str | None = None,
) -> PaperSummaryStagePackage:
    """Build fail-closed summary stage package from fields (no LLM here)."""
    complete = all(
        (
            str(headline).strip(),
            str(what_it_does).strip(),
            str(why_it_matters).strip(),
            str(analogy).strip(),
        )
    )
    if error == "not_generated":
        status = "pending"
    elif error:
        status = "failed"
    elif complete:
        status = "ready_for_review"
    else:
        status = "pending"

    diagnostics = (
        f"paper_id:{paper_id}",
        f"role:{role}",
        f"binding_id:{binding_id}",
        f"model_name:{model_name}",
        f"stage_status:{status}",
        "import_write_fail_closed",
        "optional_stage_default_off",
    )
    return PaperSummaryStagePackage(
        schema_version=SCHEMA_VERSION,
        paper_id=paper_id,
        title=title,
        abstract=abstract,
        headline=headline,
        what_it_does=what_it_does,
        why_it_matters=why_it_matters,
        analogy=analogy,
        binding_id=binding_id,
        model_name=model_name,
        role=role,
        stage_status=status,
        safety_flags=SafetyFlags(),
        diagnostics=diagnostics,
        import_eligible=False,
        graph_writes_allowed=False,
        error=error,
    )


__all__ = [
    "SCHEMA_VERSION",
    "PaperSummaryStagePackage",
    "build_paper_summary_stage",
]
