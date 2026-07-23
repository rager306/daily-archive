"""Wave B live hybrid extraction package scaffold (M254 S05).

Structural package over hybrid extraction candidates. No LLM run in this
slice, no DSPy optimizer, never import/graph write.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.wave_b_hybrid_extraction_inventory import (
    HybridExtractionCandidate,
)

SCHEMA_VERSION = "m254-wave-b-live-hybrid-extraction.v1"


@dataclass(frozen=True, slots=True)
class WaveBLiveHybridExtractionPackage:
    schema_version: str
    extraction_status: str  # blocked_gate | pending_extraction
    candidate_count: int
    empty_count: int
    sampled_count: int
    total_words: int
    samples: tuple[dict[str, Any], ...]
    diagnostics: tuple[str, ...]
    wave_b_gate_open: bool
    human_go: bool
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("live hybrid extraction cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("live hybrid extraction cannot enable DSPy")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "extraction_status": self.extraction_status,
            "candidate_count": self.candidate_count,
            "empty_count": self.empty_count,
            "sampled_count": self.sampled_count,
            "total_words": self.total_words,
            "samples": list(self.samples),
            "diagnostics": list(self.diagnostics),
            "wave_b_gate_open": self.wave_b_gate_open,
            "human_go": self.human_go,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave B live hybrid extraction scaffold; "
                "no LLM extraction yet; not DSPy; not import"
            ),
        }


def build_wave_b_live_hybrid_extraction(
    *,
    candidates: Sequence[HybridExtractionCandidate],
    wave_b_gate_open: bool,
    human_go: bool,
    sample_limit: int = 40,
) -> WaveBLiveHybridExtractionPackage:
    """Build scaffold package from inventory candidates (no LLM)."""
    all_c = tuple(candidates)
    empty = sum(1 for c in all_c if c.empty)
    total_words = sum(c.word_count for c in all_c)
    limit = max(0, int(sample_limit))
    samples = tuple(c.to_dict() for c in all_c[:limit])
    if not wave_b_gate_open or not human_go:
        status = "blocked_gate"
    else:
        status = "pending_extraction"
    diagnostics = (
        f"candidates:{len(all_c)}",
        f"empty:{empty}",
        f"total_words:{total_words}",
        f"sample_limit:{limit}",
        f"sampled:{len(samples)}",
        f"wave_b_gate_open:{wave_b_gate_open}",
        f"human_go:{human_go}",
        f"extraction_status:{status}",
        "dspy:false",
        "import_write_fail_closed",
        "scaffold_no_llm",
    )
    return WaveBLiveHybridExtractionPackage(
        schema_version=SCHEMA_VERSION,
        extraction_status=status,
        candidate_count=len(all_c),
        empty_count=empty,
        sampled_count=len(samples),
        total_words=total_words,
        samples=samples,
        diagnostics=diagnostics,
        wave_b_gate_open=wave_b_gate_open,
        human_go=human_go,
        dspy_optimizer_enabled=False,
        import_eligible=False,
        graph_writes_allowed=False,
    )


__all__ = [
    "SCHEMA_VERSION",
    "WaveBLiveHybridExtractionPackage",
    "build_wave_b_live_hybrid_extraction",
]
