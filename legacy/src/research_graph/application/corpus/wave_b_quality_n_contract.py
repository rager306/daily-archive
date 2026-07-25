"""Same-n quality contract for Wave B metrics (M271).

Ensures header / LLM / GEPA / grounding / matrix worlds report compatible
joined_count before promotion or dashboard claims. Never authorizes import.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

SCHEMA_VERSION = "wave-b-quality-n-contract.v1"


@dataclass(frozen=True, slots=True)
class QualityNContract:
    schema_version: str
    canonical_joined_count: int | None
    sources: dict[str, int | None]
    all_match: bool
    mismatches: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("quality n-contract cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "canonical_joined_count": self.canonical_joined_count,
            "sources": dict(self.sources),
            "all_match": self.all_match,
            "mismatches": list(self.mismatches),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Same-n contract for quality artifacts. Promotion and gepa_justified "
                "require matching joined_count across live header and compare worlds."
            ),
        }


def _as_int(value: Any) -> int | None:
    if value is None:
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def evaluate_quality_n_contract(
    *,
    header_n: int | None = None,
    llm_n: int | None = None,
    gepa_n: int | None = None,
    grounding_n: int | None = None,
    matrix_n: int | None = None,
    compare_n: int | None = None,
    canonical: int | None = None,
) -> QualityNContract:
    """Evaluate whether quality worlds share one joined_count.

    ``canonical`` is preferred (usually live header joined cases). When omitted,
    the first non-null among header/matrix/compare is used as canonical.
    Missing sources are ignored for match; only present values must agree.
    """
    sources: dict[str, int | None] = {
        "header": _as_int(header_n),
        "llm": _as_int(llm_n),
        "gepa": _as_int(gepa_n),
        "grounding": _as_int(grounding_n),
        "matrix": _as_int(matrix_n),
        "compare": _as_int(compare_n),
    }
    canon = _as_int(canonical)
    if canon is None:
        for key in ("header", "matrix", "compare", "gepa", "llm", "grounding"):
            if sources[key] is not None:
                canon = sources[key]
                break

    mismatches: list[str] = []
    present = {k: v for k, v in sources.items() if v is not None}
    if canon is not None:
        for name, n in present.items():
            if n != canon:
                mismatches.append(f"{name}:{n}!=canonical:{canon}")
    elif len(set(present.values())) > 1:
        # no canonical but disagree among present
        for name, n in present.items():
            mismatches.append(f"{name}:{n}")

    all_match = len(mismatches) == 0 and (
        canon is not None or len(present) <= 1
    )
    diagnostics = (
        f"canonical:{canon}",
        f"present:{len(present)}",
        f"all_match:{all_match}",
        f"mismatches:{len(mismatches)}",
        "import_write_fail_closed",
        "quality_n_contract_only",
    )
    return QualityNContract(
        schema_version=SCHEMA_VERSION,
        canonical_joined_count=canon,
        sources=sources,
        all_match=all_match,
        mismatches=tuple(mismatches),
        diagnostics=diagnostics,
    )


def extract_joined_count(payload: Mapping[str, Any] | None) -> int | None:
    """Best-effort joined_count from operator / package dicts."""
    if not isinstance(payload, Mapping):
        return None
    for key in ("joined_count", "case_count", "n"):
        if payload.get(key) is not None:
            return _as_int(payload.get(key))
    metrics = payload.get("metrics")
    if isinstance(metrics, Mapping) and metrics.get("case_count") is not None:
        return _as_int(metrics.get("case_count"))
    ctx = payload.get("context")
    if isinstance(ctx, Mapping) and ctx.get("joined_count") is not None:
        return _as_int(ctx.get("joined_count"))
    worlds = payload.get("worlds")
    if isinstance(worlds, Mapping):
        wctx = worlds.get("context")
        if isinstance(wctx, Mapping):
            return _as_int(wctx.get("joined_count"))
    return None


__all__ = [
    "SCHEMA_VERSION",
    "QualityNContract",
    "evaluate_quality_n_contract",
    "extract_joined_count",
]
