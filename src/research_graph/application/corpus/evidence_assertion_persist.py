"""Persist EvidenceAssertion staging artifacts (M281 S03).

Writes JSONL staging files only. Never authorizes import/graph writes.
"""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from research_graph.application.corpus.evidence_assertion_build import (
    build_evidence_assertion,
)
from research_graph.domain.evidence_assertion import EvidenceAssertion


def persist_evidence_assertions_jsonl(
    path: Path,
    assertions: Sequence[EvidenceAssertion | Mapping[str, Any]],
) -> dict[str, Any]:
    """Write staging JSONL; every row forced import_eligible=false."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    lines: list[str] = []
    for item in assertions:
        if isinstance(item, EvidenceAssertion):
            payload = item.to_dict()
        elif isinstance(item, Mapping):
            payload = dict(item)
        else:
            continue
        payload["import_eligible"] = False
        payload["graph_writes_allowed"] = False
        payload["staging"] = True
        lines.append(json.dumps(payload, sort_keys=True, ensure_ascii=False))
    path.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")
    return {
        "path": str(path),
        "assertion_count": len(lines),
        "import_eligible": False,
        "graph_writes_allowed": False,
        "staging": True,
    }


def build_and_persist_from_grounded_gold(
    path: Path,
    grounded_gold: Mapping[str, Any],
    *,
    body_text: str | None = None,
) -> dict[str, Any]:
    """Build staging assertions from gold entities/relations that already have spans."""
    paper_id = str(grounded_gold.get("paper_id") or "unknown").replace("arxiv:", "")
    assertions: list[EvidenceAssertion] = []
    for ent in grounded_gold.get("entities") or []:
        if not isinstance(ent, Mapping):
            continue
        spans = ent.get("spans") if isinstance(ent.get("spans"), list) else []
        if not spans:
            continue
        label = str(ent.get("label") or ent.get("text") or "")
        etype = str(ent.get("type") or "entity")
        assertions.append(
            build_evidence_assertion(
                paper_id=paper_id,
                subject=label,
                predicate="INSTANCE_OF",
                object=etype,
                claim_text=label,
                span_text=body_text or label,
                spans=spans,  # type: ignore[arg-type]
                provenance={"source": "gold_entity", "entity_id": str(ent.get("id") or "")},
            )
        )
    for rel in grounded_gold.get("relations") or []:
        if not isinstance(rel, Mapping):
            continue
        spans = rel.get("spans") if isinstance(rel.get("spans"), list) else []
        if not spans:
            continue
        pred = str(rel.get("type") or rel.get("predicate") or "RELATED")
        subj = str(rel.get("source_label") or rel.get("subject") or "")
        obj = str(rel.get("target_label") or rel.get("object") or "")
        claim = f"{subj} {pred} {obj}".strip()
        assertions.append(
            build_evidence_assertion(
                paper_id=paper_id,
                subject=subj or "unknown",
                predicate=pred,
                object=obj or "unknown",
                claim_text=claim,
                span_text=body_text or claim,
                spans=spans,  # type: ignore[arg-type]
                provenance={"source": "gold_relation", "relation_id": str(rel.get("id") or "")},
            )
        )
    return persist_evidence_assertions_jsonl(path, assertions)


__all__ = [
    "persist_evidence_assertions_jsonl",
    "build_and_persist_from_grounded_gold",
]
