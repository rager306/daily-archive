"""Evidence span resolvability rules (M276 E1.6).

Pure application checks: an assertion is resolvable only when grounded in
a SourceSpan with page, bbox, or justified char range plus artifact hash.

Never authorizes import — even resolvable claims stay import_eligible=false
until explicit user go (D127).
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True, slots=True)
class ResolvabilityVerdict:
    resolvable: bool
    reason: str
    has_page: bool = False
    has_bbox: bool = False
    has_char_range: bool = False
    has_artifact_hash: bool = False
    justified_char_only: bool = False
    import_eligible: bool = False  # always false by policy

    def to_dict(self) -> dict[str, Any]:
        return {
            "resolvable": self.resolvable,
            "reason": self.reason,
            "has_page": self.has_page,
            "has_bbox": self.has_bbox,
            "has_char_range": self.has_char_range,
            "has_artifact_hash": self.has_artifact_hash,
            "justified_char_only": self.justified_char_only,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


def _bbox_ok(bbox: Any) -> bool:
    if not isinstance(bbox, (list, tuple)) or len(bbox) != 4:
        return False
    try:
        floats = [float(x) for x in bbox]
    except (TypeError, ValueError):
        return False
    return all(f == f for f in floats)  # not NaN


def evaluate_source_span_resolvability(
    span: Mapping[str, Any] | None,
    *,
    allow_char_only_fallback: bool = True,
) -> ResolvabilityVerdict:
    """Fail-closed resolvability for a single span-like mapping.

    Resolvable when artifact_hash present AND at least one of:
    - page is int
    - bbox is 4-float
    - justified char range (char_start/char_end ints, start < end) if fallback allowed
    """
    if not isinstance(span, Mapping):
        return ResolvabilityVerdict(resolvable=False, reason="span_missing")

    artifact_hash = span.get("artifact_hash") or span.get("content_hash")
    has_hash = isinstance(artifact_hash, str) and bool(artifact_hash.strip())

    page = span.get("page")
    if page is None:
        page = span.get("page_start")
    has_page = isinstance(page, int) or (
        isinstance(page, str) and page.isdigit()
    )

    bbox = span.get("bbox")
    has_bbox = _bbox_ok(bbox)

    cs, ce = span.get("char_start"), span.get("char_end")
    has_char = False
    if isinstance(cs, int) and isinstance(ce, int) and 0 <= cs < ce:
        has_char = True

    if not has_hash:
        return ResolvabilityVerdict(
            resolvable=False,
            reason="missing_artifact_hash",
            has_page=bool(has_page),
            has_bbox=has_bbox,
            has_char_range=has_char,
            has_artifact_hash=False,
        )

    if has_page or has_bbox:
        return ResolvabilityVerdict(
            resolvable=True,
            reason="page_or_bbox",
            has_page=bool(has_page),
            has_bbox=has_bbox,
            has_char_range=has_char,
            has_artifact_hash=True,
        )

    if has_char and allow_char_only_fallback:
        return ResolvabilityVerdict(
            resolvable=True,
            reason="justified_char_only_fallback",
            has_page=False,
            has_bbox=False,
            has_char_range=True,
            has_artifact_hash=True,
            justified_char_only=True,
        )

    if has_char and not allow_char_only_fallback:
        return ResolvabilityVerdict(
            resolvable=False,
            reason="char_only_forbidden",
            has_page=False,
            has_bbox=False,
            has_char_range=True,
            has_artifact_hash=True,
        )

    return ResolvabilityVerdict(
        resolvable=False,
        reason="no_page_bbox_or_char",
        has_page=False,
        has_bbox=False,
        has_char_range=False,
        has_artifact_hash=True,
    )


def evaluate_assertion_resolvability(
    spans: Sequence[Mapping[str, Any]] | None,
    *,
    allow_char_only_fallback: bool = True,
    require_all_spans: bool = False,
) -> ResolvabilityVerdict:
    """Assertion resolvable if any (default) or all spans resolve.

    import_eligible always False regardless of resolvability.
    """
    if not spans:
        return ResolvabilityVerdict(resolvable=False, reason="no_spans")

    verdicts = [
        evaluate_source_span_resolvability(
            s, allow_char_only_fallback=allow_char_only_fallback
        )
        for s in spans
        if isinstance(s, Mapping)
    ]
    if not verdicts:
        return ResolvabilityVerdict(resolvable=False, reason="no_mapping_spans")

    ok = all(v.resolvable for v in verdicts) if require_all_spans else any(
        v.resolvable for v in verdicts
    )
    if not ok:
        reasons = sorted({v.reason for v in verdicts})
        return ResolvabilityVerdict(
            resolvable=False,
            reason="spans_unresolvable:" + ",".join(reasons),
            has_page=any(v.has_page for v in verdicts),
            has_bbox=any(v.has_bbox for v in verdicts),
            has_char_range=any(v.has_char_range for v in verdicts),
            has_artifact_hash=any(v.has_artifact_hash for v in verdicts),
        )

    primary = next(v for v in verdicts if v.resolvable)
    return ResolvabilityVerdict(
        resolvable=True,
        reason=primary.reason,
        has_page=any(v.has_page for v in verdicts),
        has_bbox=any(v.has_bbox for v in verdicts),
        has_char_range=any(v.has_char_range for v in verdicts),
        has_artifact_hash=True,
        justified_char_only=all(
            v.justified_char_only for v in verdicts if v.resolvable
        ),
    )


def resolvability_rate(
    assertions: Sequence[Mapping[str, Any]],
    *,
    spans_key: str = "spans",
    allow_char_only_fallback: bool = True,
) -> dict[str, Any]:
    """Batch rate for diagnostics. import_eligible always false."""
    total = len(assertions)
    resolvable = 0
    char_only = 0
    for row in assertions:
        if not isinstance(row, Mapping):
            continue
        spans = row.get(spans_key) or row.get("grounded_in") or []
        if not isinstance(spans, Sequence):
            continue
        v = evaluate_assertion_resolvability(
            list(spans),  # type: ignore[arg-type]
            allow_char_only_fallback=allow_char_only_fallback,
        )
        if v.resolvable:
            resolvable += 1
            if v.justified_char_only:
                char_only += 1
    rate = (resolvable / total) if total else 0.0
    return {
        "total_assertions": total,
        "resolvable_count": resolvable,
        "char_only_count": char_only,
        "resolvability_rate": rate,
        "import_eligible": False,
        "graph_writes_allowed": False,
    }


__all__ = [
    "ResolvabilityVerdict",
    "evaluate_source_span_resolvability",
    "evaluate_assertion_resolvability",
    "resolvability_rate",
]
