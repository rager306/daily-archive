#!/usr/bin/env python3
"""Probe local readiness for the M043 combined sidecar evidence run.

This probe is intentionally non-invasive: it does not fetch network resources,
start containers, or call parser services. It records whether each M033 sidecar
path is replayable from prior artifacts, locally importable, or blocked for a
live target-specific run.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shutil
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TARGET = ROOT / "artifacts" / "m043-combined-sidecar-probe" / "target-subset.json"
DEFAULT_REUSE = ROOT / "artifacts" / "m043-combined-sidecar-probe" / "m033-reuse-matrix.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m043-combined-sidecar-probe"

FindSpec = Callable[[str], object | None]
Which = Callable[[str], str | None]
Env = dict[str, str]


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def artifact_status(paths: list[str]) -> dict[str, bool]:
    return {path: (ROOT / path).exists() for path in paths}


def status_from(*, replayable: bool, live_ready: bool) -> str:
    if live_ready:
        return "live_ready"
    if replayable:
        return "replayable_prior_evidence"
    return "blocked"


def build_runtime_readiness(
    *,
    target_subset: dict[str, Any],
    reuse_matrix: dict[str, Any],
    find_spec: FindSpec = importlib.util.find_spec,
    which: Which = shutil.which,
    env: Env | None = None,
) -> dict[str, Any]:
    env = env or dict(os.environ)
    systems = reuse_matrix.get("systems", {})
    if target_subset.get("article_count") != 6:
        raise ValueError("M043 S01 target subset must contain the 6-node connected component")

    def prior_present(name: str) -> bool:
        row = systems.get(name, {})
        return bool(row.get("all_prior_artifacts_present"))

    checks: dict[str, dict[str, Any]] = {}

    checks["current_baseline"] = {
        "status": "ready",
        "role": "comparison_contracts_and_refusal_boundaries",
        "prior_artifacts_present": prior_present("current_baseline"),
        "live_run_required": False,
        "blockers": [],
    }

    grobid_url = env.get("GROBID_URL", "")
    docker_available = which("docker") is not None
    grobid_live_hint = bool(grobid_url)
    checks["grobid"] = {
        "status": status_from(replayable=prior_present("grobid"), live_ready=grobid_live_hint),
        "role": "scholarly_tei_bibliography_section_candidate",
        "prior_artifacts_present": prior_present("grobid"),
        "live_run_required_for_target_specific_output": True,
        "local_hints": {"GROBID_URL_set": grobid_live_hint, "docker_available": docker_available},
        "blockers": [] if grobid_live_hint else ["grobid_service_url_not_configured"],
    }

    docling_importable = find_spec("docling") is not None
    opendataloader_importable = find_spec("opendataloader") is not None
    checks["opendataloader_pdf"] = {
        "status": status_from(
            replayable=prior_present("opendataloader_pdf"),
            live_ready=docling_importable or opendataloader_importable,
        ),
        "role": "layout_ocr_table_coordinate_candidate",
        "prior_artifacts_present": prior_present("opendataloader_pdf"),
        "live_run_required_for_target_specific_output": True,
        "local_hints": {"docling_importable": docling_importable, "opendataloader_importable": opendataloader_importable},
        "blockers": [] if (docling_importable or opendataloader_importable) else ["opendataloader_or_docling_not_importable"],
    }

    adaptix_importable = find_spec("adaptix") is not None
    checks["adaptix"] = {
        "status": status_from(replayable=prior_present("adaptix"), live_ready=adaptix_importable),
        "role": "typed_adapter_over_fixed_parser_json",
        "prior_artifacts_present": prior_present("adaptix"),
        "live_run_required_for_target_specific_output": True,
        "local_hints": {"adaptix_importable": adaptix_importable},
        "blockers": [] if adaptix_importable else ["adaptix_not_importable"],
    }

    checks["quant_mind_patterns"] = {
        "status": "ready_pattern_only" if prior_present("quant_mind_patterns") else "blocked",
        "role": "pattern_source_not_runtime_dependency",
        "prior_artifacts_present": prior_present("quant_mind_patterns"),
        "live_run_required": False,
        "runtime_dependency_adoption_authorized": False,
        "blockers": [] if prior_present("quant_mind_patterns") else ["quant_mind_pattern_artifacts_missing"],
    }

    checks["combined_architecture"] = {
        "status": "ready_recommendation" if prior_present("combined_architecture") else "blocked",
        "role": "bounded_combined_sidecar_architecture",
        "prior_artifacts_present": prior_present("combined_architecture"),
        "live_run_required": False,
        "blockers": [] if prior_present("combined_architecture") else ["combined_architecture_artifacts_missing"],
    }

    return {
        "target_subset": "artifacts/m043-combined-sidecar-probe/target-subset.json",
        "target_article_count": target_subset["article_count"],
        "target_article_keys": target_subset["article_keys"],
        "checks": checks,
        "candidate_only": True,
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }


def render_markdown(readiness: dict[str, Any]) -> str:
    lines = [
        "# M043 Sidecar Runtime Readiness",
        "",
        f"- Target article count: {readiness['target_article_count']}",
        "- Candidate only: true",
        "- Graph writes: disabled",
        "- Production import: disabled",
        "- Fact promotion: disabled",
        "",
        "| System | Status | Prior artifacts | Blockers |",
        "|---|---|---:|---|",
    ]
    for name, check in readiness["checks"].items():
        blockers = ", ".join(check.get("blockers", [])) or "none"
        lines.append(f"| {name} | {check['status']} | {str(check.get('prior_artifacts_present', False)).lower()} | {blockers} |")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-subset", type=Path, default=DEFAULT_TARGET)
    parser.add_argument("--reuse-matrix", type=Path, default=DEFAULT_REUSE)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    args = parser.parse_args()
    readiness = build_runtime_readiness(target_subset=load_json(args.target_subset), reuse_matrix=load_json(args.reuse_matrix))
    write_json(args.output_dir / "runtime-readiness.json", readiness)
    write_text(args.output_dir / "runtime-readiness.md", render_markdown(readiness))
    sys.stdout.write(
        "m043 sidecar runtime readiness complete: "
        + ", ".join(f"{name}={check['status']}" for name, check in readiness["checks"].items())
        + "\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
