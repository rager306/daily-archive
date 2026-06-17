#!/usr/bin/env python3
"""Build the M023 final artifact-scaffold gate.

This verifier assembles redacted fixture manifests, run diagnostics, MiniMax
helper diagnostics, and benchmark reports into one stable final-gate JSON plus a
human-readable review note.  It intentionally records counters, schema versions,
review states, and blocked-operation booleans only; it never persists raw paper
text, raw model responses, embeddings, vectors, or production KG write payloads.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
for import_path in (ROOT, SRC):
    if str(import_path) not in sys.path:
        sys.path.insert(0, str(import_path))

from research_graph.papers.artifacts.metrics import (  # noqa: E402
    ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION,
    build_article_artifact_benchmark_report,
    count_raw_leakage,
    count_unsafe_authorizations,
)
from research_graph.papers.artifacts.minimax_boundary import (  # noqa: E402
    MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
    MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
    build_article_artifact_minimax_request,
    validate_article_artifact_minimax_response,
)
from research_graph.papers.artifacts.models import (  # noqa: E402
    ARTICLE_ARTIFACT_DIAGNOSTICS_SCHEMA_VERSION,
    ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION,
    ARTICLE_ARTIFACT_SCHEMA_VERSION,
    ArticleArtifactRunSummary,
    build_article_artifact_manifest_from_structure,
    build_article_artifact_run_diagnostics_artifact,
    to_json,
    validate_article_artifact_manifest,
)

FINAL_GATE_SCHEMA_VERSION = "m023-artifact-scaffold-gate.v1"
FIXTURE_DIR = ROOT / "tests" / "fixtures" / "article_artifacts"
DEFAULT_STRUCTURE_PATH = FIXTURE_DIR / "basic_article_structure.json"
DEFAULT_BENCHMARK_CASES_PATH = FIXTURE_DIR / "benchmark_cases.json"
DEFAULT_BENCHMARK_GOLD_PATH = FIXTURE_DIR / "benchmark_gold.json"

BLOCKED_OPERATION_FLAGS = {
    "kg_import_blocked": True,
    "production_import_blocked": True,
    "ladybugdb_write_blocked": True,
    "trusted_fact_promotion_blocked": True,
    "raw_payload_persistence_blocked": True,
    "model_output_persistence_blocked": True,
    "embedding_vector_persistence_blocked": True,
    "dspy_optimization_blocked_until_clean_precheck": True,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "trusted_kg_import_allowed": False,
    "import_eligible_count": 0,
    "promoted_to_fact_count": 0,
    "dspy_optimization_ran": False,
}

FORBIDDEN_SERIALIZED_FRAGMENTS = (
    "raw paper text",
    '"text":',
    '"caption_text":',
    '"raw_model_output":',
    '"raw_minimax_response":',
    '"embedding":',
    '"embeddings":',
    '"vector":',
    '"vectors":',
    '"secret":',
    '"api_key":',
    '"optimizer_trace":',
    '"trusted_kg_import_allowed": true',
    '"ladybugdb_written": true',
    '"production_import_attempted": true',
    '"model_outputs_included": true',
)


class ArtifactScaffoldGateError(ValueError):
    """Raised when the final scaffold gate cannot be built safely."""


def load_json_object(path: Path) -> dict[str, Any]:
    """Read a JSON object from ``path``."""

    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise ArtifactScaffoldGateError(f"could not read JSON artifact {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise ArtifactScaffoldGateError(f"artifact is not valid JSON {path}: {exc.msg}") from exc
    if not isinstance(payload, dict):
        raise ArtifactScaffoldGateError(f"artifact must be a JSON object: {path}")
    return payload


def build_artifact_scaffold_gate(
    *,
    structure_path: Path = DEFAULT_STRUCTURE_PATH,
    benchmark_cases_path: Path = DEFAULT_BENCHMARK_CASES_PATH,
    benchmark_gold_path: Path = DEFAULT_BENCHMARK_GOLD_PATH,
) -> dict[str, Any]:
    """Build the redacted M023 final scaffold gate JSON object."""

    structure = load_json_object(structure_path)
    benchmark_cases = load_json_object(benchmark_cases_path)
    benchmark_gold = load_json_object(benchmark_gold_path)
    manifest = build_article_artifact_manifest_from_structure(structure)
    manifest_findings = validate_article_artifact_manifest(manifest)
    if manifest_findings:
        raise ArtifactScaffoldGateError(f"fixture manifest failed validation with {len(manifest_findings)} diagnostics")

    input_hashes = {"input_structure_sha256": _sha256_file(structure_path)}
    output_paths = {
        "manifest": "generated-in-memory/fixture-paper-0001-article-artifacts.json",
        "run_summary": "generated-in-memory/article-artifacts-run-summary.json",
        "diagnostics": "generated-in-memory/article-artifacts-diagnostics.json",
    }
    run_summary = ArticleArtifactRunSummary(
        run_id=str(manifest["run_id"]),
        manifests=(manifest,),
        input_hashes=input_hashes,
        output_paths=output_paths,
    ).to_redacted_dict()
    diagnostics = build_article_artifact_run_diagnostics_artifact(
        run_id=str(manifest["run_id"]),
        manifests=(manifest,),
        input_hashes=input_hashes,
        output_paths=output_paths,
    )
    helper_diagnostics = _build_helper_diagnostics(structure)
    benchmark_report = build_article_artifact_benchmark_report(
        benchmark_cases,
        benchmark_gold,
        minimax_cases=benchmark_gold["gold"],
    )

    manifest_summary = manifest.get("summary", {}) if isinstance(manifest.get("summary"), dict) else {}
    review_states = _count_review_states(manifest)
    unsafe_counters = _unsafe_counters(
        manifest=manifest,
        run_summary=run_summary,
        diagnostics=diagnostics,
        helper_diagnostics=helper_diagnostics,
        benchmark_report=benchmark_report,
    )
    gate = {
        "schema_version": FINAL_GATE_SCHEMA_VERSION,
        "artifact_schema_version": ARTICLE_ARTIFACT_SCHEMA_VERSION,
        "run_schema_version": ARTICLE_ARTIFACT_RUN_SCHEMA_VERSION,
        "diagnostics_schema_version": ARTICLE_ARTIFACT_DIAGNOSTICS_SCHEMA_VERSION,
        "benchmark_report_schema_version": ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION,
        "status": "blocked_pending_human_review_and_ontology_milestone",
        "source_artifacts": {
            "structure_fixture": _stable_path(structure_path),
            "benchmark_cases": _stable_path(benchmark_cases_path),
            "benchmark_gold": _stable_path(benchmark_gold_path),
        },
        "manifest_summary": {
            "paper_id": manifest.get("paper_id"),
            "run_id": manifest.get("run_id"),
            "artifact_count": int(manifest_summary.get("artifact_count", 0)),
            "candidate_link_count": int(manifest_summary.get("candidate_link_count", 0)),
            "artifact_counts_by_type": dict(manifest_summary.get("artifact_counts_by_type", {})),
            "candidate_link_type_counts": dict(manifest_summary.get("candidate_link_type_counts", {})),
            "review_state_counts": review_states,
            "missing_span_count": int(manifest_summary.get("missing_span_count", 0)),
            "diagnostic_count": len(manifest.get("diagnostics", [])),
        },
        "cli_summary": {
            "run_status": "detected",
            "run_summary_artifact_count": int(run_summary.get("artifact_count", 0)),
            "run_summary_candidate_link_count": int(run_summary.get("candidate_link_count", 0)),
            "diagnostics_artifact_count": int(diagnostics.get("diagnostic_count", 0)),
            "input_hashes": dict(run_summary.get("input_hashes", {})),
            "output_paths_are_scaffold_placeholders": True,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
        },
        "minimax_helper_status": {
            "helper_schema_version": MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
            "tool_name": MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
            "request_mode": helper_diagnostics.get("request_mode"),
            "validation_status": helper_diagnostics.get("response_validation_status"),
            "provider_candidate_count": int(helper_diagnostics.get("provider_candidate_count", 0)),
            "merged_candidate_count": int(helper_diagnostics.get("merged_candidate_count", 0)),
            "helper_evidence_only": helper_diagnostics.get("helper_evidence_only") is True,
            "minimax_source_of_truth": helper_diagnostics.get("minimax_source_of_truth") is True,
            "raw_prompt_persisted": helper_diagnostics.get("raw_prompt_persisted") is True,
            "raw_response_persisted": helper_diagnostics.get("raw_response_persisted") is True,
            "diagnostic_codes": list(helper_diagnostics.get("diagnostic_codes", [])),
        },
        "benchmark_status": _benchmark_status(benchmark_report),
        "dspy_readiness": {
            "status": benchmark_report.get("dspy_status"),
            "ready": benchmark_report.get("dspy_precheck", {}).get("ready") is True,
            "selected_run": benchmark_report.get("dspy_precheck", {}).get("selected_run"),
            "blockers": list(benchmark_report.get("dspy_precheck", {}).get("blockers", [])),
            "optimization_ran": False,
            "dspy_imported_or_executed": False,
        },
        "unsafe_counters": unsafe_counters,
        "blocked_operation_flags": dict(BLOCKED_OPERATION_FLAGS),
        "recommendation": "Carry this scaffold into the next ontology and KG-design milestone only after human semantic review; do not import these candidates as facts.",
    }
    findings = verify_artifact_scaffold_gate(gate)
    gate["strict_validation"] = {
        "passed": not findings,
        "finding_count": len(findings),
        "findings": findings,
    }
    return gate


def verify_artifact_scaffold_gate(gate: dict[str, Any]) -> list[dict[str, Any]]:
    """Return redacted findings for final-gate boundary violations."""

    findings: list[dict[str, Any]] = []
    if gate.get("schema_version") != FINAL_GATE_SCHEMA_VERSION:
        findings.append({"code": "schema_version_mismatch", "json_path": "/schema_version"})
    flags = gate.get("blocked_operation_flags") if isinstance(gate.get("blocked_operation_flags"), dict) else {}
    for key, expected in BLOCKED_OPERATION_FLAGS.items():
        if flags.get(key) != expected:
            findings.append({"code": "blocked_operation_flag_mismatch", "json_path": f"/blocked_operation_flags/{key}"})
    helper = gate.get("minimax_helper_status") if isinstance(gate.get("minimax_helper_status"), dict) else {}
    if helper.get("raw_prompt_persisted") is not False:
        findings.append({"code": "helper_raw_prompt_persisted", "json_path": "/minimax_helper_status/raw_prompt_persisted"})
    if helper.get("raw_response_persisted") is not False:
        findings.append({"code": "helper_raw_response_persisted", "json_path": "/minimax_helper_status/raw_response_persisted"})
    if helper.get("minimax_source_of_truth") is not False:
        findings.append({"code": "helper_marked_source_of_truth", "json_path": "/minimax_helper_status/minimax_source_of_truth"})
    serialized = json.dumps(gate, sort_keys=True).lower()
    for fragment in FORBIDDEN_SERIALIZED_FRAGMENTS:
        if fragment.lower() in serialized:
            findings.append({"code": "forbidden_serialized_fragment", "json_path": "/", "fragment": fragment})
    return findings


def render_artifact_scaffold_review_markdown(gate: dict[str, Any]) -> str:
    """Render the final scaffold review Markdown."""

    manifest = gate["manifest_summary"]
    helper = gate["minimax_helper_status"]
    benchmark = gate["benchmark_status"]
    dspy = gate["dspy_readiness"]
    unsafe = gate["unsafe_counters"]
    flags = gate["blocked_operation_flags"]
    artifact_rows = [
        f"| {name} | {count} |" for name, count in sorted(manifest["artifact_counts_by_type"].items())
    ]
    link_rows = [
        f"| {name} | {count} |" for name, count in sorted(manifest["candidate_link_type_counts"].items())
    ]
    return "\n".join(
        [
            "# M023 Article Artifact Scaffold Gate Review",
            "",
            f"- Gate schema: `{gate['schema_version']}`",
            f"- Status: **{gate['status']}**",
            "- Safety boundary: review-only candidates; no production KG import, no LadybugDB write, no fact promotion.",
            "",
            "## Artifact Types",
            "",
            "| Artifact type | Count |",
            "|---|---:|",
            *artifact_rows,
            "",
            "## Candidate Link Semantics",
            "",
            "Candidate links are provenance-backed review hints between artifact IDs or hashed references. They are not trusted facts and stay blocked from KG import until a later ontology and review milestone accepts them.",
            "",
            "| Link type | Count |",
            "|---|---:|",
            *link_rows,
            "",
            "## Review States",
            "",
            f"- Review state counts: `{json.dumps(manifest['review_state_counts'], sort_keys=True)}`",
            f"- Missing span count: `{manifest['missing_span_count']}`",
            f"- Diagnostic count: `{manifest['diagnostic_count']}`",
            "",
            "## MiniMax Helper Status",
            "",
            f"- Tool schema: `{helper['helper_schema_version']}`",
            f"- Request mode: `{helper['request_mode']}`",
            f"- Validation status: `{helper['validation_status']}`",
            f"- Provider candidates: `{helper['provider_candidate_count']}`",
            f"- Merged candidates: `{helper['merged_candidate_count']}`",
            "- Helper output remains evidence-only and is never a source of truth.",
            "",
            "## Benchmark and DSPy Readiness",
            "",
            f"- Benchmark status: `{benchmark['status']}` across `{benchmark['run_count']}` runs.",
            f"- DSPy status: `{dspy['status']}`; optimization ran: `{str(dspy['optimization_ran']).lower()}`.",
            f"- DSPy blockers: `{json.dumps(dspy['blockers'])}`",
            "- DSPy was not imported or executed; readiness is blocked until all safety counters are clean.",
            "",
            "## No-Import Boundaries",
            "",
            f"- Blocked flags: `{json.dumps(flags, sort_keys=True)}`",
            f"- Unsafe counters: `{json.dumps(unsafe, sort_keys=True)}`",
            "- Raw payloads, model responses, embeddings, vectors, secrets, and optimizer traces are not emitted by this gate.",
            "",
            "## Recommended Next Milestone",
            "",
            "Open a follow-up ontology and KG-design milestone that defines artifact/link vocabularies, acceptance workflows, and import contracts before any graph writes are enabled.",
            "",
        ]
    )


def _build_helper_diagnostics(structure: dict[str, Any]) -> dict[str, Any]:
    request = build_article_artifact_minimax_request(structure)
    tool_input = {
        "schema_version": MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
        "source_schema_version": structure.get("schema_version"),
        "manifest_schema_version": ARTICLE_ARTIFACT_SCHEMA_VERSION,
        "input_sha256": request.diagnostics["input_sha256"],
        "artifact_hints": [],
        "helper_limit": 24,
        "minimax_source_of_truth": False,
        "promoted_to_fact": False,
        "import_eligible": False,
    }
    result = validate_article_artifact_minimax_response(
        [{"type": "tool_use", "name": MINIMAX_ARTIFACT_HELPER_TOOL_NAME, "input": tool_input}],
        structure=structure,
    )
    diagnostics = dict(request.diagnostics)
    diagnostics.update(result.diagnostics)
    diagnostics["merged_candidate_count"] = len(result.candidates)
    return diagnostics


def _benchmark_status(report: dict[str, Any]) -> dict[str, Any]:
    runs = report.get("runs", {}) if isinstance(report.get("runs"), dict) else {}
    run_status: dict[str, Any] = {}
    for name, metrics in runs.items():
        totals = metrics.get("totals", {}) if isinstance(metrics.get("totals"), dict) else {}
        macro = metrics.get("macro", {}) if isinstance(metrics.get("macro"), dict) else {}
        run_status[name] = {
            "case_count": int(metrics.get("case_count", 0)),
            "artifact_precision": float(macro.get("artifact_precision", 0.0)),
            "artifact_recall": float(macro.get("artifact_recall", 0.0)),
            "raw_leakage_count": int(totals.get("raw_leakage_count", 0)),
            "unsafe_authorization_count": int(totals.get("unsafe_authorization_count", 0)),
        }
    return {
        "status": "evaluated_redacted_fixtures",
        "report_mode": report.get("report_mode"),
        "run_count": len(run_status),
        "runs": run_status,
        "helper_delta": dict(report.get("helper_delta", {})),
    }


def _unsafe_counters(
    *,
    manifest: dict[str, Any],
    run_summary: dict[str, Any],
    diagnostics: dict[str, Any],
    helper_diagnostics: dict[str, Any],
    benchmark_report: dict[str, Any],
) -> dict[str, int]:
    runs = benchmark_report.get("runs", {}) if isinstance(benchmark_report.get("runs"), dict) else {}
    all_run_raw_leakage = sum(int(run.get("totals", {}).get("raw_leakage_count", 0)) for run in runs.values())
    all_run_unsafe_auth = sum(int(run.get("totals", {}).get("unsafe_authorization_count", 0)) for run in runs.values())
    return {
        "manifest_raw_leakage_count": count_raw_leakage(manifest),
        "manifest_unsafe_authorization_count": count_unsafe_authorizations(manifest),
        "run_summary_raw_leakage_count": count_raw_leakage(run_summary),
        "run_summary_unsafe_authorization_count": count_unsafe_authorizations(run_summary),
        "diagnostics_raw_leakage_count": count_raw_leakage(diagnostics),
        "diagnostics_unsafe_authorization_count": count_unsafe_authorizations(diagnostics),
        "helper_raw_prompt_persisted_count": int(helper_diagnostics.get("raw_prompt_persisted") is True),
        "helper_raw_response_persisted_count": int(helper_diagnostics.get("raw_response_persisted") is True),
        "helper_source_of_truth_count": int(helper_diagnostics.get("minimax_source_of_truth") is True),
        "benchmark_all_runs_raw_leakage_count": all_run_raw_leakage,
        "benchmark_all_runs_unsafe_authorization_count": all_run_unsafe_auth,
        "production_import_attempted_count": 0,
        "ladybugdb_written_count": 0,
        "trusted_kg_import_allowed_count": 0,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
    }


def _count_review_states(manifest: dict[str, Any]) -> dict[str, int]:
    counter: Counter[str] = Counter()
    for artifact in manifest.get("artifacts", []):
        if not isinstance(artifact, dict):
            continue
        state = artifact.get("review_state")
        if isinstance(state, str):
            counter[state] += 1
        for link in artifact.get("candidate_links", []):
            if isinstance(link, dict) and isinstance(link.get("review_state"), str):
                counter[str(link["review_state"])] += 1
    return dict(sorted(counter.items()))


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _stable_path(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(to_json(payload), encoding="utf-8")


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build and verify the M023 article artifact scaffold gate.")
    parser.add_argument("--structure", type=Path, default=DEFAULT_STRUCTURE_PATH)
    parser.add_argument("--benchmark-cases", type=Path, default=DEFAULT_BENCHMARK_CASES_PATH)
    parser.add_argument("--benchmark-gold", type=Path, default=DEFAULT_BENCHMARK_GOLD_PATH)
    parser.add_argument("--output-json", type=Path, required=True)
    parser.add_argument("--output-markdown", type=Path, required=True)
    parser.add_argument("--strict", action="store_true", help="Exit non-zero if final-gate boundary checks fail.")
    args = parser.parse_args(argv)

    try:
        gate = build_artifact_scaffold_gate(
            structure_path=args.structure,
            benchmark_cases_path=args.benchmark_cases,
            benchmark_gold_path=args.benchmark_gold,
        )
        markdown = render_artifact_scaffold_review_markdown(gate)
        _write_json(args.output_json, gate)
        _write_text(args.output_markdown, markdown)
    except ArtifactScaffoldGateError as exc:
        sys.stderr.write(f"artifact scaffold gate failed: {exc}\n")
        return 1

    passed = gate["strict_validation"]["passed"]
    response = {
        "status": "passed" if passed else "failed",
        "output_json": _stable_path(args.output_json),
        "output_markdown": _stable_path(args.output_markdown),
        "finding_count": gate["strict_validation"]["finding_count"],
        "dspy_status": gate["dspy_readiness"]["status"],
        "production_import_attempted": False,
        "ladybugdb_written": False,
    }
    sys.stdout.write(json.dumps(response, indent=2, sort_keys=True))
    sys.stdout.write("\n")
    if args.strict and not passed:
        return 1
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
