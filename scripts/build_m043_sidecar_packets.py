#!/usr/bin/env python3
"""Build M043 candidate-only sidecar comparison packets.

Packets summarize what each sidecar can currently contribute for the fixed M043
connected-component target. They intentionally store metadata, statuses,
blockers, artifact refs, and safety flags only. They do not run parsers, embed
raw text, write graphs, or claim import eligibility.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TARGET = ROOT / "artifacts" / "m043-combined-sidecar-probe" / "target-subset.json"
DEFAULT_SOURCE = ROOT / "artifacts" / "m043-combined-sidecar-probe" / "source-readiness.json"
DEFAULT_RUNTIME = ROOT / "artifacts" / "m043-combined-sidecar-probe" / "runtime-readiness.json"
DEFAULT_REUSE = ROOT / "artifacts" / "m043-combined-sidecar-probe" / "m033-reuse-matrix.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m043-combined-sidecar-probe"
FALSE_KEYS = ("graph_write_allowed", "promotion_allowed", "production_import_attempted", "import_eligible")
SYSTEMS = ("current_baseline", "grobid", "opendataloader_pdf", "adaptix", "quant_mind_patterns", "combined_architecture")
FORBIDDEN_PACKET_KEYS = {"raw_text", "full_text", "markdown_body", "embedding", "vector", "prompt", "completion"}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _runtime_status(runtime: dict[str, Any], system: str) -> str:
    return str(runtime.get("checks", {}).get(system, {}).get("status", "unknown"))


def _reuse_artifacts(reuse: dict[str, Any], system: str) -> list[str]:
    return list(reuse.get("systems", {}).get(system, {}).get("prior_artifacts", []))


def sidecar_status(system: str, source: dict[str, Any], runtime: dict[str, Any]) -> tuple[str, list[str]]:
    runtime_status = _runtime_status(runtime, system)
    local_pdf = int(source.get("local_pdf_count", 0)) > 0
    local_source = int(source.get("local_source_file_count", 0)) > 0
    loader = int(source.get("local_loader_file_count", 0)) > 0

    if system == "current_baseline":
        if local_source or loader:
            return "ready_contract_reference", []
        return "blocked_no_local_source", ["local_source_artifact_missing"]

    if system == "grobid":
        blockers: list[str] = []
        if runtime_status != "live_ready":
            blockers.append("grobid_live_service_not_ready")
        if not local_pdf:
            blockers.append("local_pdf_missing")
        if blockers:
            return "blocked_target_specific_run_replayable_prior_evidence", blockers
        return "ready_for_bounded_live_pdf_probe", []

    if system == "opendataloader_pdf":
        blockers = []
        if runtime_status != "live_ready":
            blockers.append("opendataloader_or_docling_not_live_ready")
        if not local_pdf:
            blockers.append("local_pdf_missing")
        if blockers:
            return "blocked_target_specific_run_replayable_prior_evidence", blockers
        return "ready_for_bounded_live_pdf_probe", []

    if system == "adaptix":
        if runtime_status != "live_ready":
            return "blocked_adapter_not_live_ready_replayable_prior_evidence", ["adaptix_not_live_ready"]
        if not local_pdf:
            return "blocked_waiting_for_target_opendataloader_fixed_json", ["target_opendataloader_fixed_json_missing"]
        return "ready_after_opendataloader_fixed_json", []

    if system == "quant_mind_patterns":
        return "ready_pattern_mapping_only", []

    if system == "combined_architecture":
        return "ready_recommendation_mapping", []

    raise ValueError(f"unknown sidecar system: {system}")


def build_packets(*, target: dict[str, Any], source_readiness: dict[str, Any], runtime: dict[str, Any], reuse: dict[str, Any]) -> dict[str, Any]:
    for payload in (target, source_readiness, runtime, reuse):
        for key in FALSE_KEYS:
            if payload.get(key) is not False and key in payload:
                raise ValueError(f"safety flag must remain false: {key}")
    source_by_key = {record["article_key"]: record for record in source_readiness.get("records", [])}
    packets: list[dict[str, Any]] = []
    status_counts: Counter[str] = Counter()
    for entry in target.get("articles", []):
        key = entry["article_key"]
        source = source_by_key.get(key)
        if source is None:
            raise ValueError(f"missing source readiness for {key}")
        sidecars: dict[str, dict[str, Any]] = {}
        for system in SYSTEMS:
            status, blockers = sidecar_status(system, source, runtime)
            status_counts[f"{system}:{status}"] += 1
            sidecars[system] = {
                "status": status,
                "blockers": blockers,
                "runtime_status": _runtime_status(runtime, system),
                "prior_artifacts": _reuse_artifacts(reuse, system),
                "candidate_only": True,
            }
        packets.append(
            {
                "article_key": key,
                "category": entry.get("m041_category"),
                "article_ref": entry.get("article_ref"),
                "catalog_path": entry.get("catalog_path"),
                "source_summary": {
                    "identity_metadata_status": source.get("identity_metadata_status"),
                    "pdf_url_present": source.get("pdf_url_present"),
                    "local_pdf_count": source.get("local_pdf_count"),
                    "local_source_file_count": source.get("local_source_file_count"),
                    "local_loader_file_count": source.get("local_loader_file_count"),
                    "linked_from_count": source.get("linked_from_count"),
                },
                "sidecars": sidecars,
                "candidate_only": True,
            }
        )

    packet = {
        "target_subset": "artifacts/m043-combined-sidecar-probe/target-subset.json",
        "article_count": len(packets),
        "systems": list(SYSTEMS),
        "packets": packets,
        "status_counts": dict(sorted(status_counts.items())),
        "forbidden_payload_fields_absent": True,
        "candidate_only": True,
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }
    assert_no_forbidden_fields(packet)
    return packet


def assert_no_forbidden_fields(value: Any) -> None:
    if isinstance(value, dict):
        overlap = FORBIDDEN_PACKET_KEYS.intersection(value)
        if overlap:
            raise ValueError(f"forbidden payload fields present: {sorted(overlap)}")
        for child in value.values():
            assert_no_forbidden_fields(child)
    elif isinstance(value, list):
        for child in value:
            assert_no_forbidden_fields(child)


def render_markdown(packet: dict[str, Any]) -> str:
    lines = [
        "# M043 Candidate Sidecar Packets",
        "",
        f"- Article count: {packet['article_count']}",
        "- Candidate only: true",
        "- Graph writes: disabled",
        "- Production import: disabled",
        "- Fact promotion: disabled",
        "- Forbidden payload fields absent: true",
        "",
        "## Status counts",
        "",
        "| Status | Count |",
        "|---|---:|",
    ]
    for status, count in packet["status_counts"].items():
        lines.append(f"| {status} | {count} |")
    lines.extend(["", "## Article packets", "", "| Article | Current | GROBID | OpenDataLoader PDF | Adaptix | quant-mind |", "|---|---|---|---|---|---|"])
    for article in packet["packets"]:
        sidecars = article["sidecars"]
        lines.append(
            "| {article} | {current} | {grobid} | {odl} | {adaptix} | {quant} |".format(
                article=article["article_key"],
                current=sidecars["current_baseline"]["status"],
                grobid=sidecars["grobid"]["status"],
                odl=sidecars["opendataloader_pdf"]["status"],
                adaptix=sidecars["adaptix"]["status"],
                quant=sidecars["quant_mind_patterns"]["status"],
            )
        )
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-subset", type=Path, default=DEFAULT_TARGET)
    parser.add_argument("--source-readiness", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--runtime-readiness", type=Path, default=DEFAULT_RUNTIME)
    parser.add_argument("--reuse-matrix", type=Path, default=DEFAULT_REUSE)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    args = parser.parse_args()
    packet = build_packets(
        target=load_json(args.target_subset),
        source_readiness=load_json(args.source_readiness),
        runtime=load_json(args.runtime_readiness),
        reuse=load_json(args.reuse_matrix),
    )
    write_json(args.output_dir / "sidecar-packets.json", packet)
    write_text(args.output_dir / "sidecar-packets.md", render_markdown(packet))
    sys.stdout.write(
        "m043 sidecar packets complete: "
        f"articles={packet['article_count']} systems={len(packet['systems'])}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
