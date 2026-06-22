"""Audit script for M054 PDF acquisition (M051 S02 T02).

Reads the acquisition log and target subset, produces a deterministic
audit report in markdown. Per M045 lesson and ADR-006 binding: emits
the 5-flag safety block on every output.

Usage:
    uv run python scripts/audit_m054_pdf_acquisition.py

Outputs:
    artifacts/m054-pdf-acquisition/audit.md
"""

from __future__ import annotations

import datetime
import hashlib
import json
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LOG_PATH = REPO_ROOT / "artifacts" / "m054-pdf-acquisition" / "acquisition-log.json"
DEFAULT_TARGET_PATH = REPO_ROOT / "artifacts" / "m054-pdf-acquisition" / "target-subset.json"
DEFAULT_AUDIT_PATH = REPO_ROOT / "artifacts" / "m054-pdf-acquisition" / "audit.md"

SCHEMA_VERSION = "m054-pdf-acquisition-audit.v1"

# 5-flag safety block (per M045 lesson + ADR-006 binding).
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _count_by_status(entries: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for entry in entries:
        status = entry.get("status", "unknown")
        counts[status] = counts.get(status, 0) + 1
    return dict(sorted(counts.items()))


def _bytes_human(n: int) -> str:
    if n < 1024:
        return f"{n} B"
    if n < 1024 * 1024:
        return f"{n / 1024:.1f} KB"
    return f"{n / (1024 * 1024):.1f} MB"


def _per_record_table(entries: list[dict[str, Any]]) -> str:
    """Markdown table: arxiv_id | status | bytes | http | attempts | error."""
    lines = [
        "| arxiv_id | status | bytes | http | attempts | error |",
        "| --- | --- | ---: | ---: | ---: | --- |",
    ]
    for entry in entries:
        arxiv_id = entry.get("article_key", "unknown")
        status = entry.get("status", "unknown")
        bytes_ = entry.get("bytes", 0)
        http = entry.get("http_status", "—")
        attempts = len(entry.get("attempts", []))
        error = entry.get("error_reason", "—") or "—"
        lines.append(
            f"| `{arxiv_id}` | {status} | {_bytes_human(bytes_)} | {http} | {attempts} | {error} |"
        )
    return "\n".join(lines)


def _safety_block() -> str:
    lines = ["```json", json.dumps(SAFETY_DEFAULTS, indent=2, sort_keys=True), "```"]
    return "\n".join(lines)


def build_audit(
    log: dict[str, Any],
    target_subset: dict[str, Any],
    *,
    log_path: Path,
    target_path: Path,
) -> str:
    """Compose the audit markdown from the log + target subset."""
    entries: list[dict[str, Any]] = log.get("entries", [])
    counts = _count_by_status(entries)
    expected = target_subset.get("records", [])
    expected_ids = {r.get("article_key") for r in expected}
    actual_ids = {e.get("article_key") for e in entries}

    missing = sorted(expected_ids - actual_ids)
    unexpected = sorted(actual_ids - expected_ids)

    total_bytes = sum(e.get("bytes", 0) for e in entries)
    acquired_bytes = sum(e.get("bytes", 0) for e in entries if e.get("status") == "acquired")
    acquired_count = counts.get("acquired", 0)
    expected_count = len(expected)
    coverage_pct = (acquired_count / expected_count * 100) if expected_count else 0.0

    # Compute log+target sha256 for audit trail integrity.
    log_digest = hashlib.sha256(log_path.read_bytes()).hexdigest()[:16]
    target_digest = hashlib.sha256(target_path.read_bytes()).hexdigest()[:16]

    parts: list[str] = []
    parts.append("# M054 PDF Acquisition Audit")
    parts.append("")
    parts.append(f"**Schema version:** `{SCHEMA_VERSION}`")
    parts.append(f"**Generated at:** {datetime.datetime.now(tz=datetime.timezone.utc).isoformat()}")
    parts.append("")
    parts.append("## Inputs")
    parts.append("")
    parts.append(f"- Acquisition log: `{log_path.relative_to(REPO_ROOT)}` (sha256:{log_digest})")
    parts.append(f"- Target subset: `{target_path.relative_to(REPO_ROOT)}` (sha256:{target_digest})")
    parts.append(f"- Records expected: {expected_count}")
    parts.append(f"- Records acquired: {acquired_count} ({coverage_pct:.1f}%)")
    parts.append(f"- Total bytes acquired: {_bytes_human(acquired_bytes)}")
    parts.append(f"- Total bytes logged: {_bytes_human(total_bytes)}")
    parts.append("")
    parts.append("## Status Counts")
    parts.append("")
    if counts:
        parts.append("| status | count |")
        parts.append("| --- | ---: |")
        for status, count in counts.items():
            parts.append(f"| `{status}` | {count} |")
    else:
        parts.append("_(no entries)_")
    parts.append("")
    parts.append("## Per-Record Table")
    parts.append("")
    parts.append(_per_record_table(entries))
    parts.append("")
    if missing:
        parts.append("## Missing Records")
        parts.append("")
        for arxiv_id in missing:
            parts.append(f"- `{arxiv_id}` (in target subset, absent from log)")
        parts.append("")
    if unexpected:
        parts.append("## Unexpected Records")
        parts.append("")
        for arxiv_id in unexpected:
            parts.append(f"- `{arxiv_id}` (in log, absent from target subset)")
        parts.append("")
    parts.append("## Safety Defaults (5-Flag Block)")
    parts.append("")
    parts.append(
        "Per M045 trajectory `prohibited-claim scan` and ADR-006 binding (agent "
        "layer is diagnostic-only, no graph writes, no promotion authority):"
    )
    parts.append("")
    parts.append(_safety_block())
    parts.append("")
    parts.append("## Next-Step Recommendation")
    parts.append("")
    if acquired_count == expected_count and acquired_count > 0:
        parts.append(
            f"All {expected_count} target records acquired. The next gate is "
            f"`M055` (live GROBID/OpenDataLoader/Adaptix pilot) on these PDFs. "
            f"Per M044 lesson, expect 0-3 of 5 to produce usable conversion "
            f"output; the rest will fail-closed with `low_quality_source` or "
            f"`missing_extraction_path` and become candidates for the chunk "
            f"repair path (M022) or a re-acquisition with an alternative source."
        )
    elif acquired_count == 0:
        parts.append(
            "0/N target records acquired. The network is unreachable, "
            "arxiv.org is rate-limiting this client, or the corpus paths "
            "are unwritable. Investigate the per-record `error_reason` "
            "field in the acquisition log; do not fall back to production "
            "import or graph writes."
        )
    else:
        parts.append(
            f"{acquired_count}/{expected_count} target records acquired. "
            f"The remaining {expected_count - acquired_count} records are "
            f"explicitly recorded with `status` in the log (blocked, "
            f"low_quality_source, or network_error). Proceed to `M055` "
            f"with the acquired subset and revisit the missing records "
            f"as a follow-up M054 slice if the audit reveals a recoverable "
            f"failure mode (e.g. rate-limit, transient network)."
        )
    parts.append("")
    parts.append("## Audit Trail")
    parts.append("")
    parts.append(
        f"- Acquisition log SHA-256 prefix: `{log_digest}` "
        f"(see `artifacts/m054-pdf-acquisition/acquisition-log.json`)"
    )
    parts.append(
        f"- Target subset SHA-256 prefix: `{target_digest}` "
        f"(see `artifacts/m054-pdf-acquisition/target-subset.json`)"
    )
    parts.append("- Audit script: `scripts/audit_m054_pdf_acquisition.py`")
    parts.append("")
    return "\n".join(parts)


def main() -> int:
    if not DEFAULT_LOG_PATH.exists():
        print(f"acquisition-log.json not found at {DEFAULT_LOG_PATH}; run acquire_linked_target_pdfs.py first")
        return 1
    log = _read_json(DEFAULT_LOG_PATH)
    if DEFAULT_TARGET_PATH.exists():
        target_subset = _read_json(DEFAULT_TARGET_PATH)
    else:
        target_subset = {"records": []}

    audit_md = build_audit(
        log,
        target_subset,
        log_path=DEFAULT_LOG_PATH,
        target_path=DEFAULT_TARGET_PATH,
    )
    DEFAULT_AUDIT_PATH.parent.mkdir(parents=True, exist_ok=True)
    DEFAULT_AUDIT_PATH.write_text(audit_md, encoding="utf-8")
    print(
        f"M054 audit written to {DEFAULT_AUDIT_PATH.relative_to(REPO_ROOT)}: "
        f"acquired={log.get('counts', {}).get('acquired', 0)}, "
        f"blocked={log.get('counts', {}).get('blocked', 0)}, "
        f"network_error={log.get('counts', {}).get('network_error', 0)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
