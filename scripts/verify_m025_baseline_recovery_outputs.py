#!/usr/bin/env python3
"""Validate M025 baseline recovery and final replay outputs.

The helper is filesystem-only and intended for T02+ closeout checks. It verifies
that generated baseline artifacts expose S08-compatible metrics/provenance,
that no-network/no-write safety flags remain false, and that final replay
summaries no longer report ``baseline_missing`` when requested.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

FALSE_SAFETY_FLAGS = {
    "graph_import_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "production_ladybugdb_write_allowed": False,
    "ladybugdb_written": False,
}
REQUIRED_EVIDENCE_TYPES = {"assets", "tables", "links", "identity"}


class BaselineOutputValidationError(RuntimeError):
    """Raised when recovered baseline or final replay outputs are invalid."""


def _looks_like_url(value: str) -> bool:
    lowered = value.lower()
    return lowered.startswith(("http://", "https://", "http:/", "https:/"))


def _reject_url_path(path: Path, label: str) -> None:
    if _looks_like_url(str(path)):
        raise BaselineOutputValidationError(f"{label} must be a local filesystem path, not a URL: {path}")


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise BaselineOutputValidationError(f"required local input is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise BaselineOutputValidationError(f"required local input is not valid JSON: {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise BaselineOutputValidationError(f"required local input must be a JSON object: {path}")
    return payload


def _validate_false_safety_flags(payload: dict[str, Any], label: str) -> list[str]:
    safety_state = payload.get("safety_state") if isinstance(payload.get("safety_state"), dict) else {}
    violations = [key for key in FALSE_SAFETY_FLAGS if safety_state.get(key) is not False]
    if violations:
        return [f"{label} has unsafe safety flag(s): {', '.join(sorted(violations))}"]
    return []


def _baseline_artifact_paths(baseline: Path) -> list[Path]:
    paths = sorted(baseline.glob("*/final.json"))
    if not paths:
        raise BaselineOutputValidationError(f"no baseline artifacts were found under {baseline}")
    return paths


def validate_baseline_artifacts(baseline: Path, *, require_no_network: bool, require_no_import_flags: bool) -> list[dict[str, Any]]:
    _reject_url_path(baseline, "baseline")
    results: list[dict[str, Any]] = []
    errors: list[str] = []
    for path in _baseline_artifact_paths(baseline):
        artifact = _load_json(path)
        label = str(path)
        metrics = artifact.get("metrics") if isinstance(artifact.get("metrics"), dict) else {}
        evidence_counts = metrics.get("evidence_counts") if isinstance(metrics.get("evidence_counts"), dict) else {}
        provenance = artifact.get("baseline_provenance") if isinstance(artifact.get("baseline_provenance"), dict) else {}
        network = artifact.get("network") if isinstance(artifact.get("network"), dict) else {}
        if "chunk_count" not in metrics:
            errors.append(f"{label} is missing metrics.chunk_count")
        if set(evidence_counts) != REQUIRED_EVIDENCE_TYPES:
            errors.append(f"{label} is missing metrics.evidence_counts for {sorted(REQUIRED_EVIDENCE_TYPES)}")
        if provenance.get("kind") != "regenerated_local_baseline":
            errors.append(f"{label} does not disclose regenerated_local_baseline provenance")
        if require_no_network and network.get("network_fetch_attempted") is not False:
            errors.append(f"{label} attempted or failed to disprove network fetches")
        if require_no_import_flags:
            errors.extend(_validate_false_safety_flags(artifact, label))
        results.append(
            {
                "path": label,
                "article_ref": artifact.get("article_ref"),
                "chunk_count": metrics.get("chunk_count"),
                "evidence_counts": evidence_counts,
                "baseline_provenance_kind": provenance.get("kind"),
            }
        )
    if errors:
        raise BaselineOutputValidationError("; ".join(errors))
    return results


def validate_final_outputs(
    *,
    final: Path | None,
    final_summary: Path | None,
    require_no_network: bool,
    require_no_import_flags: bool,
    reject_baseline_missing: bool,
) -> dict[str, Any]:
    errors: list[str] = []
    summary: dict[str, Any] = {}
    if final_summary is not None:
        _reject_url_path(final_summary, "final-summary")
        summary = _load_json(final_summary)
        counts = summary.get("baseline_comparison_counts") if isinstance(summary.get("baseline_comparison_counts"), dict) else {}
        if reject_baseline_missing and int(counts.get("baseline_missing") or 0) > 0:
            errors.append("final replay summary still contains baseline_missing comparisons")
        no_network = summary.get("no_network_proof") if isinstance(summary.get("no_network_proof"), dict) else {}
        if require_no_network and no_network.get("network_fetch_attempted") is not False:
            errors.append("final replay summary does not prove network_fetch_attempted=false")
        no_write = summary.get("no_write_safety") if isinstance(summary.get("no_write_safety"), dict) else {}
        safety_violations = no_write.get("safety_violations") if isinstance(no_write.get("safety_violations"), list) else []
        if require_no_import_flags and safety_violations:
            errors.append("final replay summary contains graph/import/write safety violations")
    artifact_count = 0
    if final is not None:
        _reject_url_path(final, "final")
        paths = sorted(final.glob("*/final.json"))
        if not paths:
            errors.append(f"no final replay artifacts were found under {final}")
        for path in paths:
            artifact_count += 1
            artifact = _load_json(path)
            comparison = artifact.get("baseline_comparison") if isinstance(artifact.get("baseline_comparison"), dict) else {}
            if reject_baseline_missing and comparison.get("category") == "baseline_missing":
                errors.append(f"{path} still has baseline_comparison.category=baseline_missing")
            network = artifact.get("network") if isinstance(artifact.get("network"), dict) else {}
            if require_no_network and network.get("network_fetch_attempted") is not False:
                errors.append(f"{path} attempted or failed to disprove network fetches")
            if require_no_import_flags:
                errors.extend(_validate_false_safety_flags(artifact, str(path)))
    if errors:
        raise BaselineOutputValidationError("; ".join(errors))
    return {
        "final_artifact_count": artifact_count,
        "final_summary_path": str(final_summary) if final_summary is not None else None,
        "baseline_comparison_counts": summary.get("baseline_comparison_counts", {}),
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--final", type=Path)
    parser.add_argument("--final-summary", type=Path)
    parser.add_argument("--require-no-network", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    parser.add_argument("--reject-baseline-missing", action="store_true")
    parser.add_argument("--write-summary", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.write_summary is not None:
            _reject_url_path(args.write_summary, "write-summary")
        baseline_results = validate_baseline_artifacts(
            args.baseline,
            require_no_network=args.require_no_network,
            require_no_import_flags=args.require_no_import_flags,
        )
        final_results = validate_final_outputs(
            final=args.final,
            final_summary=args.final_summary,
            require_no_network=args.require_no_network,
            require_no_import_flags=args.require_no_import_flags,
            reject_baseline_missing=args.reject_baseline_missing,
        )
        summary = {
            "schema_version": "m025-baseline-recovery-output-validation.v00.01",
            "baseline_artifact_count": len(baseline_results),
            "baseline_results": baseline_results,
            "final_results": final_results,
            "no_network_required": bool(args.require_no_network),
            "no_import_flags_required": bool(args.require_no_import_flags),
            "baseline_missing_rejected": bool(args.reject_baseline_missing),
        }
        if args.write_summary is not None:
            args.write_summary.parent.mkdir(parents=True, exist_ok=True)
            args.write_summary.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        sys.stdout.write(
            f"validated {len(baseline_results)} baseline artifacts; "
            f"final_artifacts={final_results['final_artifact_count']}\n"
        )
        return 0
    except BaselineOutputValidationError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
