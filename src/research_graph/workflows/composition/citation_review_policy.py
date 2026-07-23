"""M221 composition: citation review policy over hybrid body_root inventory.

Runs M220 inventory scan then pure policy evaluation. No live sidecars.
Never authorizes import/writes.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from research_graph.application.corpus.citation_review_policy import (
    DEFAULT_THRESHOLDS,
    SCHEMA_VERSION,
    CitationReviewPolicyPackage,
    CitationReviewThresholds,
    evaluate_citation_review_policy,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.workflows.composition.citation_candidate_inventory import (
    CitationInventoryRequest,
    CitationInventoryResult,
    run_citation_candidate_inventory,
)

DEFAULT_SELECTION = Path("artifacts/m213-hybrid-gate/selection-20.json")
DEFAULT_BODY_ROOT = Path("artifacts/m213-hybrid-gate/runs-live-scholarly-20")


@dataclass(frozen=True, slots=True)
class CitationReviewPolicyRequest:
    hybrid_selection_path: Path = DEFAULT_SELECTION
    body_root: Path = DEFAULT_BODY_ROOT
    output_path: Path | None = None
    repo_root: Path = field(default_factory=lambda: Path("."))
    thresholds: CitationReviewThresholds = field(default_factory=lambda: DEFAULT_THRESHOLDS)
    # When set, skip inventory scan and evaluate this package (tests).
    inventory_result: CitationInventoryResult | None = None


@dataclass(frozen=True, slots=True)
class CitationReviewPolicyResult:
    schema_version: str
    policy: CitationReviewPolicyPackage
    inventory: CitationInventoryResult
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()
    output_path: str | None = None

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("citation review policy result cannot authorize import/writes")
        if self.policy.import_eligible or self.policy.graph_writes_allowed:
            raise ValueError("policy package cannot authorize import inside result")
        if self.inventory.import_eligible or self.inventory.graph_writes_allowed:
            raise ValueError("inventory cannot authorize import inside policy result")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "policy": self.policy.to_dict(),
            "inventory": self.inventory.to_dict(),
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
            "output_path": self.output_path,
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    if path.is_file() or path.is_dir() or path.is_absolute():
        return path
    return repo_root / path


def run_citation_review_policy(
    request: CitationReviewPolicyRequest,
) -> CitationReviewPolicyResult:
    """Inventory body_root then evaluate citation review policy."""
    inv = request.inventory_result
    if inv is None:
        inv = run_citation_candidate_inventory(
            CitationInventoryRequest(
                hybrid_selection_path=request.hybrid_selection_path,
                body_root=request.body_root,
                output_path=None,
                repo_root=request.repo_root,
            )
        )
    policy = evaluate_citation_review_policy(
        inv.package, thresholds=request.thresholds
    )
    diag = (
        f"policy_verdict:{policy.verdict}",
        f"inventory_citations:{inv.package.citation_total}",
        f"headers_loaded:{inv.headers_loaded}",
        f"citations_files_loaded:{inv.citations_files_loaded}",
        "import_write_fail_closed",
        "review_required:true",
        "no_live_sidecar_start",
    )

    out_path = request.output_path
    if out_path is not None:
        out_path = _resolve(out_path, request.repo_root)
        out_path.parent.mkdir(parents=True, exist_ok=True)

    result = CitationReviewPolicyResult(
        schema_version=SCHEMA_VERSION,
        policy=policy,
        inventory=inv,
        diagnostics=diag,
        output_path=str(out_path) if out_path else None,
    )
    if out_path is not None:
        out_path.write_text(
            json.dumps(result.to_dict(), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    return result


__all__ = [
    "DEFAULT_BODY_ROOT",
    "DEFAULT_SELECTION",
    "CitationReviewPolicyRequest",
    "CitationReviewPolicyResult",
    "run_citation_review_policy",
]
