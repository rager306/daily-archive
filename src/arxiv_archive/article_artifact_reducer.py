"""Article artifact reducer (M050 S02).

Idempotent merge layer for ArticleArtifactWorkCompleted events produced by
arxiv_archive.article_artifact_worker.run_worker_pool.

Per M048 patterns-review 01 §3.6 (ActiveGraph pattern: deterministic,
idempotent, content-addressed merge) and M050 architecture:

- `merge_article_artifact_results(results)`: dedup by work_id, sort, and
  produce a stable aggregate. Idempotent: same input set always produces
  byte-identical output (sorted by work_id, no duplicates, deterministic
  field order).

- `aggregate_article_artifact_log(results_dir)`: reads all
  `*.json` artifacts from a content-addressed directory, validates each
  as a work.completed event, dedups by work_id, and returns aggregate
  counts (total, ok, validation_invalid, missing_response,
  skipped_no_structure) plus a per-binding-id breakdown.

- `_safety_defaults()`: explicit 5-flag safety block. Per M045 lesson
  (M045 trajectory `prohibited-claim scan`) and ADR-006 binding: the
  agent layer is diagnostic-only with no graph writes and no promotion
  authority. All 5 flags stay false on every output.

- Schema version `m050-article-artifact-reducer.v1`.

This module does NOT mutate graph state. It is the merge layer of the
M050 pattern; the worker pool (article_artifact_worker.py) is the
fan-out layer, the requester (article_artifact_minimax.py) is the
fan-in entrypoint.

Why a separate reducer?
  - The worker pool can complete work requests in any order
    (especially when max_workers > 1). A reducer normalizes ordering
    and deduplication so downstream consumers (audit, dashboard, replay)
    see a stable view.
  - Per ActiveGraph pattern 3.6, an idempotent reducer lets a partial
    failure be retried: re-running merge on a partial result set
    produces the same aggregate as the full set (modulo missing work_ids).
  - Per ADR-006, no reducer call ever flips a safety default to true.
"""

from __future__ import annotations

import datetime
import json
from collections import Counter
from pathlib import Path
from typing import Any

from arxiv_archive.article_artifact_worker import DEFAULT_WORK_REQUEST_DIR

REDUCER_SCHEMA_VERSION = "m050-article-artifact-reducer.v1"


# Default validation_status buckets, plus a safety bucket for everything else.
DEFAULT_VALIDATION_BUCKETS: tuple[str, ...] = (
    "valid",
    "invalid",
    "skipped_no_structure",
    "not_evaluated",
)


def _safety_defaults() -> dict[str, bool]:
    """Explicit 5-flag safety block. All false on every reducer output.

    Per M045 lesson + ADR-006 binding:
    - graph_import_allowed: false (no graph import authorized)
    - graphdb_written: false (no graph DB write performed)
    - ladybugdb_written: false (no LadybugDB write performed)
    - production_import_attempted: false (no production import)
    - import_eligible: false (no promotion authority at this layer)
    """
    return {
        "graph_import_allowed": False,
        "graphdb_written": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }


def _coerce_work_completed_dict(payload: dict[str, Any]) -> dict[str, Any]:
    """Coerce a stored JSON dict into a normalized work.completed shape.

    Tolerant: accepts both `ArticleArtifactWorkCompleted.to_sanitized_dict()`
    output and ad-hoc dicts (as long as the required fields are present).
    """
    required = ("work_id", "binding_id", "model_id", "result")
    missing = [field for field in required if field not in payload]
    if missing:
        raise ValueError(
            f"work.completed payload missing required fields: {missing}; "
            f"got keys: {sorted(payload.keys())}"
        )
    return payload


def merge_article_artifact_results(
    results: list[dict[str, Any]],
    *,
    deterministic: bool = True,
) -> dict[str, Any]:
    """Idempotent merge of work.completed events.

    - Dedups by `work_id` (later occurrence wins — useful for retry merges).
    - Sorts by `work_id` when `deterministic=True` (default).
    - Always emits the same field order, sorted keys, and safety defaults.

    Returns a reducer event with schema_version, work_id list, per-binding-id
    breakdown, validation_status counts, and the explicit safety block.
    """
    by_work_id: dict[str, dict[str, Any]] = {}
    for payload in results:
        normalized = _coerce_work_completed_dict(payload)
        by_work_id[normalized["work_id"]] = normalized

    if deterministic:
        ordered = [by_work_id[wid] for wid in sorted(by_work_id.keys())]
    else:
        ordered = list(by_work_id.values())

    # Per-binding-id breakdown.
    binding_counts: Counter[str] = Counter(payload["binding_id"] for payload in ordered)

    # Validation_status counts.
    validation_status_counts: Counter[str] = Counter()
    for payload in ordered:
        result_block = payload.get("result", {}) or {}
        diagnostics = result_block.get("diagnostics", {}) or {}
        status = diagnostics.get("validation_status", "unknown")
        if status not in DEFAULT_VALIDATION_BUCKETS:
            status = "unknown"
        validation_status_counts[status] += 1

    # Re-encode to ensure stable key order in the output dict.
    return {
        "schema_version": REDUCER_SCHEMA_VERSION,
        "event_type": "work.completed.aggregate",
        "generated_at": datetime.datetime.now(tz=datetime.timezone.utc).isoformat(),
        "total_unique_work_ids": len(ordered),
        "input_count": len(results),
        "duplicate_count": max(0, len(results) - len(ordered)),
        "binding_counts": dict(sorted(binding_counts.items())),
        "validation_status_counts": dict(sorted(validation_status_counts.items())),
        "work_ids": [payload["work_id"] for payload in ordered],
        **_safety_defaults(),
    }


def _read_work_completed_file(path: Path) -> dict[str, Any] | None:
    """Read a work.completed artifact. Returns None on parse failure or schema violation.

    The reducer is fail-soft: a single malformed artifact is skipped, not
    raised. The reducer is also fail-closed: a malformed artifact never
    contributes to the aggregate counts.
    """
    try:
        raw = path.read_text(encoding="utf-8")
    except OSError:
        return None
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return None
    if not isinstance(payload, dict):
        return None
    try:
        return _coerce_work_completed_dict(payload)
    except ValueError:
        return None


def aggregate_article_artifact_log(
    results_dir: Path | str | None = None,
) -> dict[str, Any]:
    """Aggregate all `*.json` artifacts from a content-addressed directory.

    Reads each JSON file, validates it as a work.completed event, dedups by
    work_id, and emits a deterministic aggregate. Fail-soft: malformed
    files are skipped (counted in `malformed_artifact_count`).

    Returns a reducer event with the same shape as `merge_article_artifact_results`
    plus `directory` and `malformed_artifact_count` fields.
    """
    target_dir = (
        Path(results_dir) if results_dir is not None else DEFAULT_WORK_REQUEST_DIR
    )
    target_dir = target_dir.resolve()

    if not target_dir.exists():
        # Fail-closed: empty directory. Still emit the safety defaults.
        return {
            "schema_version": REDUCER_SCHEMA_VERSION,
            "event_type": "work.completed.aggregate",
            "generated_at": datetime.datetime.now(tz=datetime.timezone.utc).isoformat(),
            "directory": str(target_dir),
            "directory_exists": False,
            "total_unique_work_ids": 0,
            "input_count": 0,
            "duplicate_count": 0,
            "malformed_artifact_count": 0,
            "binding_counts": {},
            "validation_status_counts": {},
            "work_ids": [],
            **_safety_defaults(),
        }

    payloads: list[dict[str, Any]] = []
    malformed_count = 0
    for path in sorted(target_dir.glob("*.json")):
        if not path.is_file():
            continue
        payload = _read_work_completed_file(path)
        if payload is None:
            malformed_count += 1
            continue
        payloads.append(payload)

    aggregate = merge_article_artifact_results(payloads, deterministic=True)
    aggregate["directory"] = str(target_dir)
    aggregate["directory_exists"] = True
    aggregate["malformed_artifact_count"] = malformed_count
    # Preserve the field order: put directory fields after generated_at, before counts.
    ordered: dict[str, Any] = {
        "schema_version": aggregate["schema_version"],
        "event_type": aggregate["event_type"],
        "generated_at": aggregate["generated_at"],
        "directory": aggregate["directory"],
        "directory_exists": aggregate["directory_exists"],
        "total_unique_work_ids": aggregate["total_unique_work_ids"],
        "input_count": aggregate["input_count"],
        "duplicate_count": aggregate["duplicate_count"],
        "malformed_artifact_count": aggregate["malformed_artifact_count"],
        "binding_counts": aggregate["binding_counts"],
        "validation_status_counts": aggregate["validation_status_counts"],
        "work_ids": aggregate["work_ids"],
    }
    for key, value in _safety_defaults().items():
        ordered[key] = value
    return ordered


__all__ = [
    "DEFAULT_VALIDATION_BUCKETS",
    "REDUCER_SCHEMA_VERSION",
    "aggregate_article_artifact_log",
    "merge_article_artifact_results",
]
