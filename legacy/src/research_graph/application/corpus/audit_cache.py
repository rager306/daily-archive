"""Constraint-aware audit cache key (M278 E3.4).

Key = hash(payload + spans + constraints + judge_id). Pure, deterministic.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from research_graph.application.corpus.extraction_intent_validate import hash_stable_payload


def build_audit_cache_key(
    *,
    payload: Mapping[str, Any],
    spans: Sequence[Mapping[str, Any]] | None = None,
    constraints_hash: str,
    judge_id: str = "default",
    intent_hash: str | None = None,
) -> str:
    """Stable cache key; different constraints => different key."""
    body = {
        "payload": dict(payload),
        "spans": [dict(s) for s in (spans or ()) if isinstance(s, Mapping)],
        "constraints_hash": constraints_hash,
        "judge_id": judge_id,
        "intent_hash": intent_hash,
    }
    return hash_stable_payload(body)


__all__ = ["build_audit_cache_key"]
