"""Bounded multi-paper typed extraction pilot (M201 S05).

Runs a fixed small set of preserved papers through
:class:`~research_graph.application.paper_extraction.PaperExtractionUseCase`
and returns a reproducible aggregate evidence packet (counts + per-paper
status). No graph writes. No live network in the use case itself.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field

from research_graph.application.paper_extraction import (
    PaperChunk,
    PaperExtractionRequest,
    PaperExtractionResult,
    PaperExtractionUseCase,
)
from research_graph.domain.ports import LLMClientPort


@dataclass(frozen=True)
class PilotPaper:
    """One reviewed paper for the bounded pilot."""

    source_id: str
    chunks: tuple[PaperChunk, ...]


@dataclass(frozen=True)
class PilotPaperOutcome:
    source_id: str
    status: str
    entity_count: int
    relation_count: int
    evidence_linked_count: int
    diagnostic: str | None = None


@dataclass(frozen=True)
class PilotReport:
    """Aggregate candidate + quality/failure evidence for ≤N papers."""

    paper_count: int
    done_count: int
    empty_count: int
    failed_count: int
    total_entities: int
    total_relations: int
    total_evidence_linked: int
    outcomes: tuple[PilotPaperOutcome, ...] = ()
    quality: dict[str, float] = field(default_factory=dict)
    failure_codes: tuple[str, ...] = ()
    safety: dict[str, bool] = field(
        default_factory=lambda: {
            "graph_writes_authorized": False,
            "production_import_authorized": False,
            "fact_promotion_authorized": False,
            "external_network_authorized": False,
            "llm_calls_authorized": True,
        }
    )


def run_bounded_pilot(
    papers: Sequence[PilotPaper],
    *,
    llm_provider: LLMClientPort,
    max_papers: int = 5,
) -> PilotReport:
    """Run at most ``max_papers`` papers through PaperExtractionUseCase."""
    selected = list(papers)[:max_papers]
    use_case = PaperExtractionUseCase(llm_provider=llm_provider)
    outcomes: list[PilotPaperOutcome] = []
    failure_codes: list[str] = []

    total_entities = 0
    total_relations = 0
    total_evidence = 0
    done = empty = failed = 0

    for paper in selected:
        result: PaperExtractionResult = use_case.run(
            PaperExtractionRequest(source_id=paper.source_id, chunks=paper.chunks)
        )
        outcomes.append(
            PilotPaperOutcome(
                source_id=result.source_id,
                status=result.status,
                entity_count=result.entity_count,
                relation_count=result.relation_count,
                evidence_linked_count=result.evidence_linked_count,
                diagnostic=result.diagnostic,
            )
        )
        if result.status == "done":
            done += 1
        elif result.status == "empty":
            empty += 1
        else:
            failed += 1
            if result.diagnostic:
                failure_codes.append(result.diagnostic)
        total_entities += result.entity_count
        total_relations += result.relation_count
        total_evidence += result.evidence_linked_count

    denom = max(done, 1)
    quality = {
        "entities_per_done_paper": total_entities / denom,
        "relations_per_done_paper": total_relations / denom,
        "evidence_link_rate": (total_evidence / max(total_entities + total_relations, 1)),
        "done_rate": done / max(len(selected), 1),
    }

    return PilotReport(
        paper_count=len(selected),
        done_count=done,
        empty_count=empty,
        failed_count=failed,
        total_entities=total_entities,
        total_relations=total_relations,
        total_evidence_linked=total_evidence,
        outcomes=tuple(outcomes),
        quality=quality,
        failure_codes=tuple(failure_codes),
    )


__all__ = [
    "PilotPaper",
    "PilotPaperOutcome",
    "PilotReport",
    "run_bounded_pilot",
]
