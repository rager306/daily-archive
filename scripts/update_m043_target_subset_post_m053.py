#!/usr/bin/env python3
"""Update M043 target-subset.json with M053 GROBID pilot outcomes.

Reads the M053 summary and per-PDF packets, then records a
`grobid_outcome_post_m053` block on each M043 article. Records outside the
M053 scope are explicitly marked `not_in_m053_scope`. This script is
fail-closed: it never marks import eligibility and keeps all five safety
defaults false.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SUMMARY_PATH = REPO_ROOT / "artifacts" / "m053-grobid-pilot" / "summary.json"
DEFAULT_TARGET_PATH = REPO_ROOT / "artifacts" / "m043-combined-sidecar-probe" / "target-subset.json"

SCHEMA_VERSION = "m043-target-subset-post-m053.v1"
SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_import_allowed": False,
    "graphdb_written": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "import_eligible": False,
}


def utc_now() -> str:
    return dt.datetime.now(tz=dt.timezone.utc).isoformat()


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _relative(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def _packet_path(summary_packet: dict[str, Any], per_pdf_dir: Path) -> Path:
    raw_path = summary_packet.get("packet_path")
    if raw_path:
        candidate = Path(raw_path)
        if not candidate.is_absolute():
            candidate = REPO_ROOT / candidate
        if candidate.exists():
            return candidate
    paper_id = str(summary_packet.get("paper_id") or summary_packet.get("arxiv_id"))
    return per_pdf_dir / f"{paper_id}.json"


def load_packets_by_paper_id(summary: dict[str, Any], per_pdf_dir: Path) -> dict[str, dict[str, Any]]:
    packets: dict[str, dict[str, Any]] = {}
    for summary_packet in summary.get("packets", []):
        packet = _read_json(_packet_path(summary_packet, per_pdf_dir))
        paper_id = str(packet.get("paper_id") or packet.get("arxiv_id") or summary_packet.get("paper_id"))
        packets[paper_id] = packet
    return packets


def _attempt_count(packet: dict[str, Any]) -> int:
    attempts = packet.get("attempts")
    if isinstance(attempts, list):
        return len(attempts)
    if isinstance(attempts, int):
        return attempts
    return 0


def _outcome_from_packet(packet: dict[str, Any]) -> dict[str, Any]:
    return {
        "status": packet.get("status", "unknown"),
        "tei_size_bytes": int(packet.get("tei_size_bytes") or 0),
        "ref_count": int(packet.get("ref_count") or 0),
        "body_element_count": int(packet.get("body_element_count") or 0),
        "http_status": packet.get("http_status"),
        "attempts": _attempt_count(packet),
        "m022_repair_candidate": bool(packet.get("m022_repair_candidate", False)),
        "safety_defaults": dict(SAFETY_DEFAULTS),
    }


def _not_in_scope_outcome() -> dict[str, Any]:
    return {
        "status": "not_in_m053_scope",
        "tei_size_bytes": 0,
        "ref_count": 0,
        "body_element_count": 0,
        "http_status": None,
        "attempts": 0,
        "m022_repair_candidate": False,
        "safety_defaults": dict(SAFETY_DEFAULTS),
    }


def update_m043_target_subset(
    target_subset: dict[str, Any],
    packets_by_paper_id: dict[str, dict[str, Any]],
    *,
    summary_path: Path,
) -> dict[str, Any]:
    """Return an updated M043 target subset preserving existing fields."""
    updated = dict(target_subset)
    updated["articles"] = []
    updated["last_updated_at"] = utc_now()
    updated["update_source"] = _relative(summary_path)
    updated["update_schema_version"] = SCHEMA_VERSION
    updated["safety_defaults"] = dict(SAFETY_DEFAULTS)

    for article in target_subset.get("articles", []):
        article_copy = dict(article)
        article_key = str(article.get("article_key"))
        packet = packets_by_paper_id.get(article_key)
        article_copy["grobid_outcome_post_m053"] = _outcome_from_packet(packet) if packet else _not_in_scope_outcome()
        updated["articles"].append(article_copy)

    return updated


def write_updated_target(summary_path: Path, per_pdf_dir: Path, target_path: Path, output_path: Path) -> dict[str, Any]:
    summary = _read_json(summary_path)
    packets_by_paper_id = load_packets_by_paper_id(summary, per_pdf_dir)
    target_subset = _read_json(target_path)
    updated = update_m043_target_subset(target_subset, packets_by_paper_id, summary_path=summary_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(updated, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8")
    return updated


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Update M043 target subset with M053 GROBID outcomes")
    parser.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY_PATH)
    parser.add_argument("--per-pdf-dir", type=Path, default=None)
    parser.add_argument("--target", type=Path, default=DEFAULT_TARGET_PATH)
    parser.add_argument("--output", type=Path, default=None)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    summary_path = args.summary
    per_pdf_dir = args.per_pdf_dir or summary_path.parent
    target_path = args.target
    output_path = args.output or target_path
    if not summary_path.exists():
        print(f"M053 summary not found at {summary_path}")
        return 1
    if not per_pdf_dir.exists():
        print(f"M053 per-PDF directory not found at {per_pdf_dir}")
        return 1
    if not target_path.exists():
        print(f"M043 target-subset not found at {target_path}")
        return 1

    updated = write_updated_target(summary_path, per_pdf_dir, target_path, output_path)
    scoped_count = sum(
        1
        for article in updated.get("articles", [])
        if article.get("grobid_outcome_post_m053", {}).get("status") != "not_in_m053_scope"
    )
    not_in_scope_count = sum(
        1
        for article in updated.get("articles", [])
        if article.get("grobid_outcome_post_m053", {}).get("status") == "not_in_m053_scope"
    )
    print(
        f"M043 target-subset updated: total={len(updated.get('articles', []))}, "
        f"grobid_outcomes_post_m053={scoped_count}, not_in_m053_scope={not_in_scope_count}"
    )
    print(f"Safety defaults: {json.dumps(SAFETY_DEFAULTS, sort_keys=True)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
