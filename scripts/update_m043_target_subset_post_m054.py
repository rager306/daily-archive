"""Update M043 target-subset.json with M054 PDF acquisition results.

M051 S02 T03: close M045 next_gate by recording, for each M043 record,
the actual local-pdf state after M054 acquisition ran.

Inputs:
- artifacts/m054-pdf-acquisition/acquisition-log.json (M054 results)
- artifacts/m043-combined-sidecar-probe/target-subset.json (M043 manifest)

Output:
- artifacts/m043-combined-sidecar-probe/target-subset.json
  (in-place update, preserving all existing fields; adds a new
  `local_pdf_present_post_m054` field per record)

The script is idempotent: re-running with the same acquisition log
produces the same M043 file (modulo `last_updated_at`).

Per M045 lesson + ADR-006: emits the 5-flag safety block in stdout
and never claims import eligibility.
"""

from __future__ import annotations

import datetime
import json
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LOG_PATH = REPO_ROOT / "artifacts" / "m054-pdf-acquisition" / "acquisition-log.json"
DEFAULT_TARGET_PATH = REPO_ROOT / "artifacts" / "m043-combined-sidecar-probe" / "target-subset.json"

SCHEMA_VERSION = "m043-target-subset-post-m054.v1"

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _index_acquisition_by_key(log: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Map article_key -> acquisition entry from the M054 log."""
    return {
        entry["article_key"]: entry for entry in log.get("entries", []) if "article_key" in entry
    }


def update_m043_target_subset(
    target_subset: dict[str, Any],
    acquisition_by_key: dict[str, dict[str, Any]],
    *,
    log_path: Path,
) -> dict[str, Any]:
    """Add a per-record `local_pdf_present_post_m054` block to M043.

    Returns a new dict (does not mutate the input). The original fields
    of each article are preserved; only the new `local_pdf_present_post_m054`
    field is added.
    """
    new_target = {
        "article_count": target_subset.get("article_count"),
        "article_keys": list(target_subset.get("article_keys", [])),
        "articles": [],
        "last_updated_at": datetime.datetime.now(tz=datetime.timezone.utc).isoformat(),
        "update_source": str(log_path.relative_to(REPO_ROOT)),
        "update_schema_version": SCHEMA_VERSION,
        "safety_defaults": dict(SAFETY_DEFAULTS),
    }

    for article in target_subset.get("articles", []):
        article_copy = dict(article)
        article_key = article.get("article_key")
        if article_key in acquisition_by_key:
            entry = acquisition_by_key[article_key]
            article_copy["local_pdf_present_post_m054"] = {
                "status": entry.get("status", "unknown"),
                "http_status": entry.get("http_status"),
                "bytes": entry.get("bytes", 0),
                "sha256": entry.get("sha256"),
                "local_path": entry.get("local_path"),
                "url": entry.get("url"),
                "acquired_at": entry.get("completed_at"),
                "attempts": len(entry.get("attempts", [])),
            }
        else:
            # Not in M054 acquisition scope; record absent status.
            article_copy["local_pdf_present_post_m054"] = {
                "status": "not_in_m054_scope",
                "http_status": None,
                "bytes": 0,
                "sha256": None,
                "local_path": None,
                "url": None,
                "acquired_at": None,
                "attempts": 0,
            }
        new_target["articles"].append(article_copy)

    return new_target


def main() -> int:
    if not DEFAULT_LOG_PATH.exists():
        print(
            f"acquisition log not found at {DEFAULT_LOG_PATH}; run acquire_linked_target_pdfs.py first"
        )
        return 1
    if not DEFAULT_TARGET_PATH.exists():
        print(f"M043 target-subset not found at {DEFAULT_TARGET_PATH}")
        return 1

    log = _read_json(DEFAULT_LOG_PATH)
    target_subset = _read_json(DEFAULT_TARGET_PATH)
    acquisition_by_key = _index_acquisition_by_key(log)

    new_target = update_m043_target_subset(
        target_subset, acquisition_by_key, log_path=DEFAULT_LOG_PATH
    )

    DEFAULT_TARGET_PATH.write_text(
        json.dumps(new_target, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

    acquired_count = sum(
        1
        for a in new_target["articles"]
        if a.get("local_pdf_present_post_m054", {}).get("status") == "acquired"
    )
    not_in_scope_count = sum(
        1
        for a in new_target["articles"]
        if a.get("local_pdf_present_post_m054", {}).get("status") == "not_in_m054_scope"
    )
    print(
        f"M043 target-subset updated: total={len(new_target['articles'])}, "
        f"acquired_post_m054={acquired_count}, not_in_m054_scope={not_in_scope_count}"
    )
    print(f"Safety defaults: {json.dumps(SAFETY_DEFAULTS, sort_keys=True)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
