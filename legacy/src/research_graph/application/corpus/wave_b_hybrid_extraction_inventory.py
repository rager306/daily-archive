"""Wave B hybrid-body extraction candidate inventory.

Metadata-only readiness for extraction quality on hybrid bodies:
paper_id, path, char/word counts. No LLM, no DSPy, no import.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.application.corpus.etl_preprocess_fleet_audit import (
    HybridBodyRef,
    discover_unique_hybrid_bodies,
)

SCHEMA_VERSION = "wave-b-hybrid-extraction-inventory.v1"


@dataclass(frozen=True, slots=True)
class HybridExtractionCandidate:
    paper_id: str
    path: str
    body_root: str
    char_count: int
    word_count: int
    empty: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "path": self.path,
            "body_root": self.body_root,
            "char_count": self.char_count,
            "word_count": self.word_count,
            "empty": self.empty,
            "import_eligible": False,
        }


@dataclass(frozen=True, slots=True)
class WaveBHybridExtractionInventoryPackage:
    schema_version: str
    candidate_count: int
    empty_count: int
    total_chars: int
    total_words: int
    candidates: tuple[HybridExtractionCandidate, ...]
    diagnostics: tuple[str, ...]
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("hybrid extraction inventory cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("hybrid extraction inventory cannot enable DSPy")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "candidate_count": self.candidate_count,
            "empty_count": self.empty_count,
            "total_chars": self.total_chars,
            "total_words": self.total_words,
            "candidates": [c.to_dict() for c in self.candidates],
            "diagnostics": list(self.diagnostics),
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave B hybrid extraction candidates metadata only; "
                "not LLM run; not DSPy; not import"
            ),
        }


def _stats_for_ref(ref: HybridBodyRef) -> HybridExtractionCandidate:
    try:
        text = ref.path.read_text(encoding="utf-8")
    except OSError:
        text = ""
    words = len(text.split()) if text.strip() else 0
    return HybridExtractionCandidate(
        paper_id=ref.paper_id,
        path=str(ref.path),
        body_root=ref.body_root,
        char_count=len(text),
        word_count=words,
        empty=not text.strip(),
    )


def inventory_hybrid_extraction_candidates(
    *,
    body_roots: Sequence[Path],
    sample_limit: int = 40,
) -> WaveBHybridExtractionInventoryPackage:
    """Discover unique hybrid bodies and report extraction-candidate stats."""
    refs = discover_unique_hybrid_bodies(body_roots)
    all_candidates = tuple(_stats_for_ref(r) for r in refs)
    empty = sum(1 for c in all_candidates if c.empty)
    total_chars = sum(c.char_count for c in all_candidates)
    total_words = sum(c.word_count for c in all_candidates)
    samples = all_candidates[: max(0, sample_limit)]
    diagnostics = (
        f"unique_bodies:{len(all_candidates)}",
        f"empty:{empty}",
        f"total_chars:{total_chars}",
        f"total_words:{total_words}",
        f"sample_limit:{sample_limit}",
        "dspy:false",
        "import_write_fail_closed",
        "wave_b_hybrid_inventory_only",
        "no_llm_extraction",
    )
    return WaveBHybridExtractionInventoryPackage(
        schema_version=SCHEMA_VERSION,
        candidate_count=len(all_candidates),
        empty_count=empty,
        total_chars=total_chars,
        total_words=total_words,
        candidates=samples,
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "HybridExtractionCandidate",
    "WaveBHybridExtractionInventoryPackage",
    "inventory_hybrid_extraction_candidates",
]
