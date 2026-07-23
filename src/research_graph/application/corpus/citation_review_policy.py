"""Pure citation candidate review policy over M220 inventory (M221).

Maps inventory coverage to human-review readiness verdicts.
Never authorizes import/writes. Not graph truth. Not human approval.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal

from research_graph.application.corpus.citation_candidate_inventory import (
    CitationInventoryPackage,
)

SCHEMA_VERSION = "m221-citation-review-policy.v1"

CitationReviewVerdict = Literal[
    "ready_for_human_review",
    "repair",
    "blocked",
]


@dataclass(frozen=True, slots=True)
class CitationReviewThresholds:
    """Fail-closed coverage thresholds for candidate review readiness.

    idno is advisory by default: live GROBID TEI idno_coverage ~0.40 on
    selection-20 is expected; hard-failing on it would block all papers.
    """

    min_papers: int = 1
    min_citation_total: int = 1
    min_title_coverage: float = 0.90
    min_author_coverage: float = 0.80
    # Fraction of papers that must have a citations file present.
    min_citations_file_fraction: float = 0.90
    # Advisory only unless enforce_idno is True.
    min_idno_coverage_advisory: float = 0.30
    enforce_idno: bool = False
    max_empty_title_fraction: float = 0.10

    def to_dict(self) -> dict[str, Any]:
        return {
            "min_papers": self.min_papers,
            "min_citation_total": self.min_citation_total,
            "min_title_coverage": self.min_title_coverage,
            "min_author_coverage": self.min_author_coverage,
            "min_citations_file_fraction": self.min_citations_file_fraction,
            "min_idno_coverage_advisory": self.min_idno_coverage_advisory,
            "enforce_idno": self.enforce_idno,
            "max_empty_title_fraction": self.max_empty_title_fraction,
        }


DEFAULT_THRESHOLDS = CitationReviewThresholds()


@dataclass(frozen=True, slots=True)
class CitationReviewPolicyPackage:
    schema_version: str
    verdict: CitationReviewVerdict
    thresholds: CitationReviewThresholds
    title_coverage: float
    author_coverage: float
    idno_coverage: float
    empty_title_fraction: float
    citations_file_fraction: float
    paper_count: int
    citation_total: int
    checks: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    review_required: bool = True
    note: str = (
        "policy readiness for human review only; not import; not graph write"
    )

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("citation review policy cannot authorize import/writes")
        if self.verdict == "ready_for_human_review" and not self.review_required:
            raise ValueError("ready verdict still requires human review")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "verdict": self.verdict,
            "thresholds": self.thresholds.to_dict(),
            "title_coverage": self.title_coverage,
            "author_coverage": self.author_coverage,
            "idno_coverage": self.idno_coverage,
            "empty_title_fraction": self.empty_title_fraction,
            "citations_file_fraction": self.citations_file_fraction,
            "paper_count": self.paper_count,
            "citation_total": self.citation_total,
            "checks": list(self.checks),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "review_required": True,
            "note": self.note,
        }


def _coverage(numer: int, denom: int) -> float:
    if denom <= 0:
        return 0.0
    return numer / denom


def evaluate_citation_review_policy(
    inventory: CitationInventoryPackage,
    *,
    thresholds: CitationReviewThresholds | None = None,
) -> CitationReviewPolicyPackage:
    """Evaluate inventory against thresholds → human-review readiness.

    Verdict ladder:
    - blocked: no papers / no cites / zero citations files
    - repair: hard coverage thresholds failed
    - ready_for_human_review: hard thresholds met (import still false)
    """
    th = thresholds or DEFAULT_THRESHOLDS
    n_cite = max(inventory.citation_total, 0)
    n_paper = max(inventory.paper_count, 0)
    title_cov = _coverage(inventory.with_title, n_cite)
    author_cov = _coverage(inventory.with_authors, n_cite)
    idno_cov = _coverage(inventory.with_idno, n_cite)
    empty_frac = _coverage(inventory.empty_title, n_cite)
    file_frac = _coverage(inventory.papers_with_citations_file, n_paper)

    checks: list[str] = []
    hard_fail: list[str] = []
    block: list[str] = []
    advisory: list[str] = []

    if n_paper < th.min_papers:
        block.append(f"papers:{n_paper}<{th.min_papers}")
    else:
        checks.append(f"papers_ok:{n_paper}>={th.min_papers}")

    if n_cite < th.min_citation_total:
        block.append(f"citations:{n_cite}<{th.min_citation_total}")
    else:
        checks.append(f"citations_ok:{n_cite}>={th.min_citation_total}")

    if inventory.papers_with_citations_file <= 0 and n_paper > 0:
        block.append("no_citations_files")
    elif file_frac < th.min_citations_file_fraction:
        hard_fail.append(
            f"citations_file_fraction:{file_frac:.4f}<{th.min_citations_file_fraction}"
        )
    else:
        checks.append(f"citations_file_fraction_ok:{file_frac:.4f}")

    if title_cov < th.min_title_coverage:
        hard_fail.append(f"title_coverage:{title_cov:.4f}<{th.min_title_coverage}")
    else:
        checks.append(f"title_coverage_ok:{title_cov:.4f}")

    if author_cov < th.min_author_coverage:
        hard_fail.append(f"author_coverage:{author_cov:.4f}<{th.min_author_coverage}")
    else:
        checks.append(f"author_coverage_ok:{author_cov:.4f}")

    if empty_frac > th.max_empty_title_fraction:
        hard_fail.append(
            f"empty_title_fraction:{empty_frac:.4f}>{th.max_empty_title_fraction}"
        )
    else:
        checks.append(f"empty_title_fraction_ok:{empty_frac:.4f}")

    if idno_cov < th.min_idno_coverage_advisory:
        msg = f"idno_coverage_advisory:{idno_cov:.4f}<{th.min_idno_coverage_advisory}"
        if th.enforce_idno:
            hard_fail.append(msg)
        else:
            advisory.append(msg)
            checks.append(f"idno_advisory_only:{idno_cov:.4f}")
    else:
        checks.append(f"idno_coverage_ok:{idno_cov:.4f}")

    if block:
        verdict: CitationReviewVerdict = "blocked"
    elif hard_fail:
        verdict = "repair"
    else:
        verdict = "ready_for_human_review"

    diagnostics = (
        f"verdict:{verdict}",
        "import_write_fail_closed",
        "review_required:true",
        "not_graph_import",
        *block,
        *hard_fail,
        *advisory,
    )

    return CitationReviewPolicyPackage(
        schema_version=SCHEMA_VERSION,
        verdict=verdict,
        thresholds=th,
        title_coverage=title_cov,
        author_coverage=author_cov,
        idno_coverage=idno_cov,
        empty_title_fraction=empty_frac,
        citations_file_fraction=file_frac,
        paper_count=n_paper,
        citation_total=n_cite,
        checks=tuple(checks),
        diagnostics=diagnostics,
    )


__all__ = [
    "DEFAULT_THRESHOLDS",
    "SCHEMA_VERSION",
    "CitationReviewPolicyPackage",
    "CitationReviewThresholds",
    "CitationReviewVerdict",
    "evaluate_citation_review_policy",
]
