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
        raise BaselineOutputValidationError(
            f"{label} must be a local filesystem path, not a URL: {path}"
        )


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise BaselineOutputValidationError(f"required local input is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise BaselineOutputValidationError(
            f"required local input is not valid JSON: {path}: {exc}"
        ) from exc
    if not isinstance(payload, dict):
        raise BaselineOutputValidationError(f"required local input must be a JSON object: {path}")
    return payload


def _validate_false_safety_flags(payload: dict[str, Any], label: str) -> list[str]:
    safety_state = (
        payload.get("safety_state") if isinstance(payload.get("safety_state"), dict) else {}
    )
    violations = [key for key in FALSE_SAFETY_FLAGS if safety_state.get(key) is not False]  # ty:ignore[unresolved-attribute]
    if violations:
        return [f"{label} has unsafe safety flag(s): {', '.join(sorted(violations))}"]
    return []


def _baseline_artifact_paths(baseline: Path) -> list[Path]:
    paths = sorted(baseline.glob("*/final.json"))
    if not paths:
        raise BaselineOutputValidationError(f"no baseline artifacts were found under {baseline}")
    return paths


def validate_baseline_artifacts(
    baseline: Path, *, require_no_network: bool, require_no_import_flags: bool
) -> list[dict[str, Any]]:
    _reject_url_path(baseline, "baseline")
    results: list[dict[str, Any]] = []
    errors: list[str] = []
    for path in _baseline_artifact_paths(baseline):
        artifact = _load_json(path)
        label = str(path)
        metrics = artifact.get("metrics") if isinstance(artifact.get("metrics"), dict) else {}
        evidence_counts = (
            metrics.get("evidence_counts")  # ty:ignore[unresolved-attribute]
            if isinstance(metrics.get("evidence_counts"), dict)  # ty:ignore[unresolved-attribute]
            else {}
        )
        provenance = (
            artifact.get("baseline_provenance")
            if isinstance(artifact.get("baseline_provenance"), dict)
            else {}
        )
        network = artifact.get("network") if isinstance(artifact.get("network"), dict) else {}
        if "chunk_count" not in metrics:  # ty:ignore[unsupported-operator]
            errors.append(f"{label} is missing metrics.chunk_count")
        if set(evidence_counts) != REQUIRED_EVIDENCE_TYPES:  # ty:ignore[invalid-argument-type]
            errors.append(
                f"{label} is missing metrics.evidence_counts for {sorted(REQUIRED_EVIDENCE_TYPES)}"
            )
        if provenance.get("kind") != "regenerated_local_baseline":  # ty:ignore[unresolved-attribute]
            errors.append(f"{label} does not disclose regenerated_local_baseline provenance")
        if require_no_network and network.get("network_fetch_attempted") is not False:  # ty:ignore[unresolved-attribute]
            errors.append(f"{label} attempted or failed to disprove network fetches")
        if require_no_import_flags:
            errors.extend(_validate_false_safety_flags(artifact, label))
        results.append(
            {
                "path": label,
                "article_ref": artifact.get("article_ref"),
                "chunk_count": metrics.get("chunk_count"),  # ty:ignore[unresolved-attribute]
                "evidence_counts": evidence_counts,
                "baseline_provenance_kind": provenance.get("kind"),  # ty:ignore[unresolved-attribute]
            }
        )
    if errors:
        raise BaselineOutputValidationError("; ".join(errors))
    return results


def validate_final_outputs(
    *,
    final: Path | None,
    final_summary: Path | None,
    expect_article_count: int | None,
    require_no_network: bool,
    require_no_import_flags: bool,
    reject_baseline_missing: bool,
    require_ready: bool,
) -> dict[str, Any]:
    errors: list[str] = []
    summary: dict[str, Any] = {}
    if final_summary is not None:
        _reject_url_path(final_summary, "final-summary")
        summary = _load_json(final_summary)
        if (
            expect_article_count is not None
            and summary.get("article_count") != expect_article_count
        ):
            errors.append(
                f"final replay summary article_count={summary.get('article_count')} does not match expected {expect_article_count}"
            )
        counts = (
            summary.get("baseline_comparison_counts")
            if isinstance(summary.get("baseline_comparison_counts"), dict)
            else {}
        )
        if reject_baseline_missing and int(counts.get("baseline_missing") or 0) > 0:  # ty:ignore[unresolved-attribute]
            errors.append("final replay summary still contains baseline_missing comparisons")
        no_network = (
            summary.get("no_network_proof")
            if isinstance(summary.get("no_network_proof"), dict)
            else {}
        )
        if require_no_network and no_network.get("network_fetch_attempted") is not False:  # ty:ignore[unresolved-attribute]
            errors.append("final replay summary does not prove network_fetch_attempted=false")
        no_write = (
            summary.get("no_write_safety")
            if isinstance(summary.get("no_write_safety"), dict)
            else {}
        )
        safety_violations = (
            no_write.get("safety_violations")  # ty:ignore[unresolved-attribute]
            if isinstance(no_write.get("safety_violations"), list)  # ty:ignore[unresolved-attribute]
            else []
        )
        if require_no_import_flags and safety_violations:
            errors.append("final replay summary contains graph/import/write safety violations")
        readiness = summary.get("readiness") if isinstance(summary.get("readiness"), dict) else {}
        if require_ready and readiness.get("larger_preprocessing_validation_ready") is not True:  # ty:ignore[unresolved-attribute]
            errors.append(
                "final replay summary does not mark larger_preprocessing_validation_ready=true"
            )
        if readiness.get("graph_readiness_claim") is not False:  # ty:ignore[unresolved-attribute]
            errors.append("final replay summary must keep graph_readiness_claim=false")
    artifact_count = 0
    if final is not None:
        _reject_url_path(final, "final")
        paths = sorted(final.glob("*/final.json"))
        if not paths:
            errors.append(f"no final replay artifacts were found under {final}")
        for path in paths:
            artifact_count += 1
            artifact = _load_json(path)
            comparison = (
                artifact.get("baseline_comparison")
                if isinstance(artifact.get("baseline_comparison"), dict)
                else {}
            )
            if reject_baseline_missing and comparison.get("category") == "baseline_missing":  # ty:ignore[unresolved-attribute]
                errors.append(f"{path} still has baseline_comparison.category=baseline_missing")
            network = artifact.get("network") if isinstance(artifact.get("network"), dict) else {}
            if require_no_network and network.get("network_fetch_attempted") is not False:  # ty:ignore[unresolved-attribute]
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
    parser.add_argument("--baseline", "--baseline-dir", dest="baseline", type=Path)
    parser.add_argument("--baseline-summary", type=Path)
    parser.add_argument("--baseline-events", type=Path)
    parser.add_argument("--expect-article-count", type=int)
    parser.add_argument("--final", type=Path)
    parser.add_argument("--final-summary", type=Path)
    parser.add_argument("--require-no-network", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    parser.add_argument(
        "--reject-baseline-missing",
        "--expect-no-baseline-missing",
        dest="reject_baseline_missing",
        action="store_true",
    )
    parser.add_argument("--readiness-decision", type=Path)
    parser.add_argument("--require-ready", action="store_true")
    parser.add_argument("--write-summary", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.write_summary is not None:
            _reject_url_path(args.write_summary, "write-summary")
        baseline_results = (
            validate_baseline_artifacts(
                args.baseline,
                require_no_network=args.require_no_network,
                require_no_import_flags=args.require_no_import_flags,
            )
            if args.baseline is not None
            else []
        )
        if (
            args.baseline is None
            and args.final is None
            and args.final_summary is None
            and args.readiness_decision is None
        ):
            raise BaselineOutputValidationError(
                "at least one of --baseline, --final, --final-summary, or --readiness-decision is required"
            )
        final_results = validate_final_outputs(
            final=args.final,
            final_summary=args.final_summary,
            expect_article_count=args.expect_article_count,
            require_no_network=args.require_no_network,
            require_no_import_flags=args.require_no_import_flags,
            reject_baseline_missing=args.reject_baseline_missing,
            require_ready=args.require_ready,
        )
        recovery_summary: dict[str, Any] = {}
        recovery_event_count = 0
        if (
            args.expect_article_count is not None
            and args.baseline is not None
            and len(baseline_results) != args.expect_article_count
        ):
            raise BaselineOutputValidationError(
                f"expected {args.expect_article_count} baseline artifacts, found {len(baseline_results)}"
            )
        if args.baseline_summary is not None:
            _reject_url_path(args.baseline_summary, "baseline-summary")
            recovery_summary = _load_json(args.baseline_summary)
            if (
                args.expect_article_count is not None
                and recovery_summary.get("article_count") != args.expect_article_count
            ):
                raise BaselineOutputValidationError(
                    f"baseline summary article_count={recovery_summary.get('article_count')} does not match "
                    f"expected {args.expect_article_count}"
                )
            readiness = (
                recovery_summary.get("readiness")
                if isinstance(recovery_summary.get("readiness"), dict)
                else {}
            )
            if readiness.get("baseline_recovery_completed") is not True:  # ty:ignore[unresolved-attribute]
                raise BaselineOutputValidationError(
                    "baseline summary does not mark baseline_recovery_completed=true"
                )
            no_network = (
                recovery_summary.get("no_network_proof")
                if isinstance(recovery_summary.get("no_network_proof"), dict)
                else {}
            )
            if args.require_no_network and no_network.get("network_fetch_attempted") is not False:  # ty:ignore[unresolved-attribute]
                raise BaselineOutputValidationError(
                    "baseline summary does not prove network_fetch_attempted=false"
                )
            no_write = (
                recovery_summary.get("no_write_safety")
                if isinstance(recovery_summary.get("no_write_safety"), dict)
                else {}
            )
            safety_violations = (
                no_write.get("safety_violations")  # ty:ignore[unresolved-attribute]
                if isinstance(no_write.get("safety_violations"), list)  # ty:ignore[unresolved-attribute]
                else []
            )
            if args.require_no_import_flags and safety_violations:
                raise BaselineOutputValidationError(
                    "baseline summary contains graph/import/write safety violations"
                )
        if args.baseline_events is not None:
            _reject_url_path(args.baseline_events, "baseline-events")
            try:
                event_lines = [
                    line
                    for line in args.baseline_events.read_text(encoding="utf-8").splitlines()
                    if line.strip()
                ]
            except FileNotFoundError as exc:
                raise BaselineOutputValidationError(
                    f"required local input is missing: {args.baseline_events}"
                ) from exc
            events: list[dict[str, Any]] = []
            for line_number, line in enumerate(event_lines, start=1):
                try:
                    event = json.loads(line)
                except json.JSONDecodeError as exc:
                    raise BaselineOutputValidationError(
                        f"baseline events file is not valid JSONL at line {line_number}: {args.baseline_events}: {exc}"
                    ) from exc
                if not isinstance(event, dict):
                    raise BaselineOutputValidationError(
                        f"baseline events file line {line_number} must be a JSON object: {args.baseline_events}"
                    )
                events.append(event)
            recovery_event_count = len(events)
            completed_count = sum(
                1
                for event in events
                if event.get("event_type") == "baseline_recovery.article_completed"
            )
            if (
                args.expect_article_count is not None
                and completed_count != args.expect_article_count
            ):
                raise BaselineOutputValidationError(
                    f"baseline events completed article count={completed_count} does not match expected {args.expect_article_count}"
                )
            if args.require_no_network and any(
                event.get("network_fetch_attempted") is not False for event in events
            ):
                raise BaselineOutputValidationError(
                    "baseline events do not prove network_fetch_attempted=false"
                )
            if args.require_no_import_flags:
                unsafe_events = [
                    event
                    for event in events
                    if event.get("production_import_attempted") is not False
                    or event.get("ladybugdb_written") is not False
                ]
                if unsafe_events:
                    raise BaselineOutputValidationError(
                        "baseline events contain graph/import/write safety violations"
                    )
        if args.readiness_decision is not None:
            _reject_url_path(args.readiness_decision, "readiness-decision")
            decision = _load_json(args.readiness_decision)
            if (
                args.require_ready
                and decision.get("larger_preprocessing_validation_ready") is not True
            ):
                raise BaselineOutputValidationError(
                    "readiness decision does not mark larger_preprocessing_validation_ready=true"
                )
            if args.require_ready and decision.get("decision") != "ready":
                raise BaselineOutputValidationError("readiness decision is not ready")
            if decision.get("graph_readiness_claim") is not False:
                raise BaselineOutputValidationError(
                    "readiness decision must keep graph_readiness_claim=false"
                )
            blockers = (
                decision.get("blockers") if isinstance(decision.get("blockers"), list) else []
            )
            if args.reject_baseline_missing and "baseline_missing" in blockers:  # ty:ignore[unsupported-operator]
                raise BaselineOutputValidationError(
                    "readiness decision still contains baseline_missing blocker"
                )
            evidence = (
                decision.get("evidence") if isinstance(decision.get("evidence"), dict) else {}
            )
            if (
                args.expect_article_count is not None
                and evidence.get("article_count") != args.expect_article_count  # ty:ignore[unresolved-attribute]
            ):
                raise BaselineOutputValidationError(
                    f"readiness decision article_count={evidence.get('article_count')} does not match expected {args.expect_article_count}"  # ty:ignore[unresolved-attribute]
                )
            no_network = (
                evidence.get("no_network_proof")  # ty:ignore[unresolved-attribute]
                if isinstance(evidence.get("no_network_proof"), dict)  # ty:ignore[unresolved-attribute]
                else {}
            )
            if args.require_no_network and no_network.get("network_fetch_attempted") is not False:  # ty:ignore[unresolved-attribute]
                raise BaselineOutputValidationError(
                    "readiness decision does not prove network_fetch_attempted=false"
                )
            no_write = (
                evidence.get("no_write_safety")  # ty:ignore[unresolved-attribute]
                if isinstance(evidence.get("no_write_safety"), dict)  # ty:ignore[unresolved-attribute]
                else {}
            )
            safety_violations = (
                no_write.get("safety_violations")  # ty:ignore[unresolved-attribute]
                if isinstance(no_write.get("safety_violations"), list)  # ty:ignore[unresolved-attribute]
                else []
            )
            if args.require_no_import_flags and safety_violations:
                raise BaselineOutputValidationError(
                    "readiness decision contains graph/import/write safety violations"
                )

        summary = {
            "schema_version": "m025-baseline-recovery-output-validation.v00.01",
            "baseline_artifact_count": len(baseline_results),
            "baseline_results": baseline_results,
            "baseline_summary_path": str(args.baseline_summary)
            if args.baseline_summary is not None
            else None,
            "baseline_summary_article_count": recovery_summary.get("article_count"),
            "baseline_events_path": str(args.baseline_events)
            if args.baseline_events is not None
            else None,
            "baseline_event_count": recovery_event_count,
            "expected_article_count": args.expect_article_count,
            "final_results": final_results,
            "no_network_required": bool(args.require_no_network),
            "no_import_flags_required": bool(args.require_no_import_flags),
            "baseline_missing_rejected": bool(args.reject_baseline_missing),
        }
        if args.write_summary is not None:
            args.write_summary.parent.mkdir(parents=True, exist_ok=True)
            args.write_summary.write_text(
                json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
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
