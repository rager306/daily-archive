"""Wave B hybrid-body statistical extraction package (M255 S01).

Deterministic, statistical-first candidate extraction over hybrid body text:
token-frequency keywords + window co-occurrence → RELATED_TO candidates.

No LLM, no DSPy, never import/graph write. Application-pure (stdlib only).
"""

from __future__ import annotations

import re
from collections import Counter
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

SCHEMA_VERSION = "m255-wave-b-hybrid-statistical-extraction.v1"
FLEET_SCHEMA_VERSION = "m255-wave-b-hybrid-statistical-fleet.v1"

_TOKEN_RE = re.compile(r"[A-Za-z\u0400-\u04FF][A-Za-z\u0400-\u04FF'\-]{2,}")
_STOP = frozenset(
    """
    the and for that with this from are was were been being have has had
    not but you all can her was one our out day get has him his how its
    may new now old see two way who boy did its let put say she too use
    into than then them they will what when which your about after also
    just more most other some such only over also very paper model models
    using based approach method methods results show shown figure table
    """.split()
)


def _content_keywords(text: str, *, top_k: int = 16) -> list[str]:
    tokens = [
        m.group(0).casefold()
        for m in _TOKEN_RE.finditer(text)
        if m.group(0).casefold() not in _STOP
    ]
    if not tokens:
        return []
    return [w for w, _ in Counter(tokens).most_common(top_k)]


def _co_occurrence(
    keywords: Sequence[str],
    text: str,
    *,
    min_count: int = 2,
    window_tokens: int = 40,
) -> list[tuple[str, str, int]]:
    """Count keyword pair co-occurrence inside sliding token windows."""
    if len(keywords) < 2 or not text.strip():
        return []
    kw_set = set(keywords)
    tokens = [m.group(0).casefold() for m in _TOKEN_RE.finditer(text)]
    pair_counts: Counter[tuple[str, str]] = Counter()
    if not tokens:
        return []
    step = max(1, window_tokens // 2)
    for start in range(0, len(tokens), step):
        window = tokens[start : start + window_tokens]
        present = sorted({t for t in window if t in kw_set})
        for i, a in enumerate(present):
            for b in present[i + 1 :]:
                pair_counts[(a, b)] += 1
    out: list[tuple[str, str, int]] = []
    for (a, b), count in pair_counts.most_common():
        if count >= min_count:
            out.append((a, b, count))
    return out


@dataclass(frozen=True, slots=True)
class HybridStatisticalExtractionPackage:
    schema_version: str
    paper_id: str
    body_path: str | None
    word_count: int
    keyword_count: int
    cooc_pair_count: int
    candidate_relation_count: int
    keywords: tuple[dict[str, Any], ...]
    candidate_relations: tuple[dict[str, Any], ...]
    keyword_source: str
    extraction_status: str  # empty_body | statistical_ready
    diagnostics: tuple[str, ...]
    llm_used: bool = False
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("hybrid statistical extraction cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("hybrid statistical extraction cannot enable DSPy")
        if self.llm_used:
            raise ValueError("hybrid statistical extraction must not claim LLM use")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "paper_id": self.paper_id,
            "body_path": self.body_path,
            "word_count": self.word_count,
            "keyword_count": self.keyword_count,
            "cooc_pair_count": self.cooc_pair_count,
            "candidate_relation_count": self.candidate_relation_count,
            "keywords": list(self.keywords),
            "candidate_relations": list(self.candidate_relations),
            "keyword_source": self.keyword_source,
            "extraction_status": self.extraction_status,
            "diagnostics": list(self.diagnostics),
            "llm_used": False,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave B statistical-first hybrid extraction; "
                "token-frequency + co-occurrence only; not LLM; not DSPy; not import"
            ),
        }


@dataclass(frozen=True, slots=True)
class HybridStatisticalFleetPackage:
    schema_version: str
    fleet_status: str  # blocked_gate | sampled
    paper_count: int
    statistical_ready_count: int
    empty_count: int
    total_keywords: int
    total_candidate_relations: int
    total_words: int
    packages: tuple[dict[str, Any], ...]
    diagnostics: tuple[str, ...]
    wave_b_gate_open: bool
    human_go: bool
    llm_used: bool = False
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("hybrid statistical fleet cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("hybrid statistical fleet cannot enable DSPy")
        if self.llm_used:
            raise ValueError("hybrid statistical fleet must not claim LLM use")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "fleet_status": self.fleet_status,
            "paper_count": self.paper_count,
            "statistical_ready_count": self.statistical_ready_count,
            "empty_count": self.empty_count,
            "total_keywords": self.total_keywords,
            "total_candidate_relations": self.total_candidate_relations,
            "total_words": self.total_words,
            "packages": list(self.packages),
            "diagnostics": list(self.diagnostics),
            "wave_b_gate_open": self.wave_b_gate_open,
            "human_go": self.human_go,
            "llm_used": False,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave B statistical fleet over hybrid bodies; "
                "not LLM; not DSPy; not import"
            ),
        }


def build_hybrid_statistical_extraction(
    *,
    paper_id: str,
    body_text: str,
    body_path: str | None = None,
    top_k: int = 16,
    min_cooc: int = 2,
) -> HybridStatisticalExtractionPackage:
    """Build fail-closed statistical extraction package for one hybrid body."""
    text = body_text or ""
    words = text.split()
    word_count = len(words)
    if not text.strip():
        return HybridStatisticalExtractionPackage(
            schema_version=SCHEMA_VERSION,
            paper_id=paper_id,
            body_path=body_path,
            word_count=0,
            keyword_count=0,
            cooc_pair_count=0,
            candidate_relation_count=0,
            keywords=(),
            candidate_relations=(),
            keyword_source="token_frequency",
            extraction_status="empty_body",
            diagnostics=(
                f"paper_id:{paper_id}",
                "extraction_status:empty_body",
                "llm:false",
                "dspy:false",
                "import_write_fail_closed",
            ),
            llm_used=False,
            dspy_optimizer_enabled=False,
            import_eligible=False,
            graph_writes_allowed=False,
        )

    kws = _content_keywords(text, top_k=top_k)
    # scores: inverse rank as pseudo-score for diagnostics only
    keywords = tuple(
        {
            "keyword": k,
            "rank": i + 1,
            "score": round(1.0 / (i + 1), 4),
            "import_eligible": False,
        }
        for i, k in enumerate(kws)
    )
    pairs = _co_occurrence(kws, text, min_count=min_cooc)
    relations = tuple(
        {
            "relation_id": f"{paper_id}:rel:cooc:{idx}:{a}:{b}",
            "relation_type": "RELATED_TO",
            "from_keyword": a,
            "to_keyword": b,
            "cooc_count": count,
            "confidence": round(min(1.0, count / 10.0), 4),
            "import_eligible": False,
        }
        for idx, (a, b, count) in enumerate(pairs)
    )
    diagnostics = (
        f"paper_id:{paper_id}",
        f"word_count:{word_count}",
        f"keyword_count:{len(keywords)}",
        f"cooc_pair_count:{len(pairs)}",
        f"candidate_relation_count:{len(relations)}",
        "keyword_source:token_frequency",
        "extraction_status:statistical_ready",
        "llm:false",
        "dspy:false",
        "import_write_fail_closed",
        "statistical_first",
    )
    return HybridStatisticalExtractionPackage(
        schema_version=SCHEMA_VERSION,
        paper_id=paper_id,
        body_path=body_path,
        word_count=word_count,
        keyword_count=len(keywords),
        cooc_pair_count=len(pairs),
        candidate_relation_count=len(relations),
        keywords=keywords,
        candidate_relations=relations,
        keyword_source="token_frequency",
        extraction_status="statistical_ready",
        diagnostics=diagnostics,
        llm_used=False,
        dspy_optimizer_enabled=False,
        import_eligible=False,
        graph_writes_allowed=False,
    )


def build_hybrid_statistical_fleet(
    *,
    packages: Sequence[HybridStatisticalExtractionPackage],
    wave_b_gate_open: bool,
    human_go: bool,
) -> HybridStatisticalFleetPackage:
    """Aggregate per-paper statistical packages under Wave B gate."""
    pkgs = tuple(packages)
    empty = sum(1 for p in pkgs if p.extraction_status == "empty_body")
    ready = sum(1 for p in pkgs if p.extraction_status == "statistical_ready")
    status = "sampled" if (wave_b_gate_open and human_go) else "blocked_gate"
    diagnostics = (
        f"paper_count:{len(pkgs)}",
        f"statistical_ready:{ready}",
        f"empty:{empty}",
        f"total_keywords:{sum(p.keyword_count for p in pkgs)}",
        f"total_candidate_relations:{sum(p.candidate_relation_count for p in pkgs)}",
        f"wave_b_gate_open:{wave_b_gate_open}",
        f"human_go:{human_go}",
        f"fleet_status:{status}",
        "llm:false",
        "dspy:false",
        "import_write_fail_closed",
    )
    return HybridStatisticalFleetPackage(
        schema_version=FLEET_SCHEMA_VERSION,
        fleet_status=status,
        paper_count=len(pkgs),
        statistical_ready_count=ready,
        empty_count=empty,
        total_keywords=sum(p.keyword_count for p in pkgs),
        total_candidate_relations=sum(p.candidate_relation_count for p in pkgs),
        total_words=sum(p.word_count for p in pkgs),
        packages=tuple(p.to_dict() for p in pkgs),
        diagnostics=diagnostics,
        wave_b_gate_open=wave_b_gate_open,
        human_go=human_go,
        llm_used=False,
        dspy_optimizer_enabled=False,
        import_eligible=False,
        graph_writes_allowed=False,
    )


__all__ = [
    "FLEET_SCHEMA_VERSION",
    "SCHEMA_VERSION",
    "HybridStatisticalExtractionPackage",
    "HybridStatisticalFleetPackage",
    "build_hybrid_statistical_extraction",
    "build_hybrid_statistical_fleet",
]
