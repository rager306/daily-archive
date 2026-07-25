"""Join gold fixtures with hybrid bodies and score resolvability (M281 S02).

Application pure join + metric. Never import.
"""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.application.corpus.canary_resolvability_metric import (
    evaluate_canary_resolvability,
)
from research_graph.application.corpus.gold_char_span_grounding import (
    attach_char_spans_to_gold_case,
)

SCHEMA_VERSION = "canary-gold-hybrid-join.v1"


def normalize_paper_id(value: str) -> str:
    s = str(value or "").replace("arxiv:", "").strip()
    # strip trailing version for matching but keep original elsewhere
    return s


def load_gold_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    text = Path(path).read_text(encoding="utf-8")
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        obj = json.loads(line)
        if isinstance(obj, dict):
            rows.append(obj)
    return rows


def index_hybrid_bodies(body_roots: Sequence[Path]) -> dict[str, Path]:
    """Map normalized paper_id → hybrid.body.md path (first hit)."""
    out: dict[str, Path] = {}
    for root in body_roots:
        root_p = Path(root)
        if not root_p.is_dir():
            continue
        for path in sorted(root_p.rglob("*.hybrid.body.md")):
            name = path.name
            if name.endswith(".hybrid.body.md"):
                stem = name[: -len(".hybrid.body.md")]
            else:
                stem = path.stem
            # parent dir often paper_id
            parent = path.parent.parent.name if path.parent.name == "body" else path.parent.name
            for key in (stem, parent):
                if key in {"body", "original", ""}:
                    continue
                nk = normalize_paper_id(key)
                if nk and nk not in out:
                    out[nk] = path
                # also without version suffix
                base = nk.split("v")[0] if "v" in nk else nk
                if base and base not in out:
                    out[base] = path
    return out


def join_gold_with_hybrid_bodies(
    gold_rows: Sequence[Mapping[str, Any]],
    body_index: Mapping[str, Path],
) -> list[dict[str, Any]]:
    """Return cases with body_text + gold for rows that have a hybrid body."""
    joined: list[dict[str, Any]] = []
    for gold in gold_rows:
        if not isinstance(gold, Mapping):
            continue
        pid = normalize_paper_id(str(gold.get("paper_id") or ""))
        base = pid.split("v")[0] if "v" in pid else pid
        path = body_index.get(pid) or body_index.get(base)
        if path is None:
            continue
        try:
            body = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        joined.append(
            {
                "case_id": str(gold.get("case_id") or f"case:{pid}"),
                "paper_id": pid,
                "body_text": body,
                "body_path": str(path),
                "gold": dict(gold),
            }
        )
    return joined


@dataclass(frozen=True, slots=True)
class CanaryGoldHybridJoinPackage:
    schema_version: str
    gold_total: int
    joined_count: int
    entity_grounded: int
    entity_total: int
    relation_grounded: int
    relation_total: int
    resolvability: dict[str, Any]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("canary gold hybrid join cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "gold_total": self.gold_total,
            "joined_count": self.joined_count,
            "entity_grounded": self.entity_grounded,
            "entity_total": self.entity_total,
            "relation_grounded": self.relation_grounded,
            "relation_total": self.relation_total,
            "resolvability": dict(self.resolvability),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Real join of gold fixtures to hybrid bodies with char-span grounding. "
                "Page/bbox still absent (justified char-only). Never import."
            ),
        }


def evaluate_joined_canary_resolvability(
    *,
    gold_rows: Sequence[Mapping[str, Any]],
    body_roots: Sequence[Path],
    target_rate: float = 0.95,
) -> CanaryGoldHybridJoinPackage:
    """Join, attach char spans, run resolvability metric."""
    index = index_hybrid_bodies(body_roots)
    joined = join_gold_with_hybrid_bodies(gold_rows, index)
    grounded_golds: list[dict[str, Any]] = []
    eg = et = rg = rt = 0
    for case in joined:
        result = attach_char_spans_to_gold_case(
            gold=case["gold"],
            body_text=str(case.get("body_text") or ""),
            case_id=str(case.get("case_id") or ""),
            paper_id=str(case.get("paper_id") or ""),
        )
        eg += result.entity_grounded
        et += result.entity_total
        rg += result.relation_grounded
        rt += result.relation_total
        grounded_golds.append(result.gold)

    metric = evaluate_canary_resolvability(
        grounded_golds,
        target_rate=target_rate,
        expand_gold=True,
        metric_mode="real_gold_hybrid_join",
        demo_metric=False,
        min_n=10,
    )
    diagnostics = (
        f"gold_total:{len(gold_rows)}",
        f"joined_count:{len(joined)}",
        f"body_index_size:{len(index)}",
        f"entity_grounded:{eg}/{et}",
        f"relation_grounded:{rg}/{rt}",
        f"resolvability_rate:{metric.resolvability_rate}",
        f"target_met:{str(metric.target_met).lower()}",
        "mode:char_span_from_hybrid_body",
        "import_write_fail_closed",
    )
    return CanaryGoldHybridJoinPackage(
        schema_version=SCHEMA_VERSION,
        gold_total=len(gold_rows),
        joined_count=len(joined),
        entity_grounded=eg,
        entity_total=et,
        relation_grounded=rg,
        relation_total=rt,
        resolvability=metric.to_dict(),
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "normalize_paper_id",
    "load_gold_jsonl",
    "index_hybrid_bodies",
    "join_gold_with_hybrid_bodies",
    "CanaryGoldHybridJoinPackage",
    "evaluate_joined_canary_resolvability",
]
