#!/usr/bin/env python3
"""Continuity audit for the M036 real-corpus no-write smoke artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

FALSE_SAFETY_KEYS = (
    "graph_write_allowed",
    "promotion_allowed",
    "production_import_attempted",
    "import_eligible",
)
CONTINUITY_SCHEMA_VERSION = "real-corpus-continuity.v1"
PER_ARTICLE_ARTIFACTS = (
    "candidate.json",
    "review_packet.json",
    "review_trace.json",
    "queue_inspect.json",
    "readiness_handoff.json",
    "continuity.json",
)
FORBIDDEN_PAYLOAD_TERMS = (
    "api_key",
    "secret_value",
    "bearer ",
    "x-api-key",
    "embedding_payload",
    "vector_payload",
    "chunk_text_payload",
    "paper_text_payload",
    "claim_text_payload",
)


def emit(message: str) -> None:
    sys.stdout.write(f"{message}\n")


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def false_flags(payload: dict[str, Any]) -> bool:
    return all(payload.get(key) is False for key in FALSE_SAFETY_KEYS)


def safety_summary(run_summary: dict[str, Any]) -> dict[str, bool]:
    return {key: bool(run_summary.get(key)) for key in FALSE_SAFETY_KEYS}


def artifact_payload_safe(path: Path) -> bool:
    text = path.read_text(encoding="utf-8", errors="ignore").lower()
    return not any(term in text for term in FORBIDDEN_PAYLOAD_TERMS)


def validate_continuity(article: dict[str, Any], continuity: dict[str, Any]) -> dict[str, Any]:
    candidate_id = article.get("candidate_id")
    if continuity.get("schema_version") != CONTINUITY_SCHEMA_VERSION:
        raise ValueError(f"article {candidate_id} continuity schema version is invalid")
    if continuity.get("metadata_only") is not True:
        raise ValueError(f"article {candidate_id} continuity metadata_only must be true")
    safety_flags = continuity.get("safety_flags")
    if not isinstance(safety_flags, dict) or not false_flags(safety_flags):
        raise ValueError(
            f"article {candidate_id} continuity contains true write/import/promotion flag"
        )
    import_eligibility = continuity.get("import_eligibility")
    if (
        not isinstance(import_eligibility, dict)
        or import_eligibility.get("import_eligible") is not False
    ):
        raise ValueError(f"article {candidate_id} continuity claims import eligibility")
    loader_evidence = continuity.get("loader_evidence")
    if not isinstance(loader_evidence, dict):
        raise ValueError(f"article {candidate_id} continuity missing loader_evidence")
    loader_status = loader_evidence.get("status")
    if loader_status not in {"present", "absent_explicit"}:
        raise ValueError(f"article {candidate_id} continuity loader status is invalid")
    diagnostics = continuity.get("diagnostics")
    if not isinstance(diagnostics, list):
        raise ValueError(f"article {candidate_id} continuity diagnostics must be a list")
    if loader_status == "absent_explicit" and "loader_evidence_absent_explicit" not in diagnostics:
        raise ValueError(f"article {candidate_id} continuity must diagnose explicit loader absence")
    if "safety_flags_missing_or_not_false" in diagnostics:
        raise ValueError(f"article {candidate_id} continuity contains legacy safety diagnostic")
    return continuity


def inspect_article_artifacts(article: dict[str, Any]) -> dict[str, Any]:
    article_dir = Path(str(article["artifact_dir"]))
    missing = [name for name in PER_ARTICLE_ARTIFACTS if not (article_dir / name).exists()]
    unsafe_payload_files = [
        str(path)
        for path in article_dir.glob("*.json")
        if path.is_file() and not artifact_payload_safe(path)
    ]
    continuity = None
    continuity_path = article_dir / "continuity.json"
    if continuity_path.exists():
        continuity = validate_continuity(article, load_json(continuity_path))
    return {
        "artifact_dir": str(article_dir),
        "missing_artifacts": missing,
        "unsafe_payload_files": unsafe_payload_files,
        "continuity": continuity,
    }


def blockers_from(
    diagnostics: Counter[str],
    *,
    missing_artifacts: int,
    unsafe_payload_files: int,
    missing_continuity_artifacts: int,
) -> list[str]:
    blockers: list[str] = []
    if diagnostics.get("missing_loader_evidence", 0):
        blockers.append("missing_loader_evidence")
    if diagnostics.get("safety_flags_missing_or_not_false", 0):
        blockers.append("legacy_or_missing_article_safety_flags")
    if missing_continuity_artifacts:
        blockers.append("missing_continuity_metadata")
    if missing_artifacts:
        blockers.append("missing_smoke_artifacts")
    if unsafe_payload_files:
        blockers.append("unsafe_payload_terms")
    return blockers


def audit_smoke(manifest_path: Path, run_dir: Path) -> dict[str, Any]:
    manifest = load_json(manifest_path)
    run_summary = load_json(run_dir / "summary.json")
    manifest_articles = manifest.get("articles")
    run_articles = run_summary.get("articles")
    if not isinstance(manifest_articles, list) or not isinstance(run_articles, list):
        raise ValueError("manifest and run summary must contain article lists")

    article_artifact_status = [inspect_article_artifacts(article) for article in run_articles]
    continuity_records = [
        item["continuity"] for item in article_artifact_status if item.get("continuity")
    ]
    diagnostics = Counter(
        diagnostic
        for continuity in continuity_records
        for diagnostic in continuity.get("diagnostics", [])
    )
    missing_artifacts = sum(len(item["missing_artifacts"]) for item in article_artifact_status)
    unsafe_payload_files = sum(
        len(item["unsafe_payload_files"]) for item in article_artifact_status
    )
    missing_continuity_artifacts = sum(
        1 for item in article_artifact_status if "continuity.json" in item["missing_artifacts"]
    )
    articles_with_source_refs = sum(
        1
        for continuity in continuity_records
        if continuity["source_evidence"]["status"] == "present"
    )
    articles_with_loader_refs = sum(
        1
        for continuity in continuity_records
        if continuity["loader_evidence"]["status"] == "present"
    )
    articles_with_explicit_loader_absence = sum(
        1
        for continuity in continuity_records
        if continuity["loader_evidence"]["status"] == "absent_explicit"
    )
    ready_count = sum(1 for article in run_articles if article.get("queue_status") == "ready")

    safety = safety_summary(run_summary)
    if any(safety.values()):
        raise ValueError("run summary contains true write/import/promotion flag")
    for article in run_articles:
        if not false_flags(article):
            raise ValueError(
                f"article {article.get('candidate_id')} contains true write/import/promotion flag"
            )
        article_flags = article.get("safety_flags")
        if not isinstance(article_flags, dict) or not false_flags(article_flags):
            raise ValueError(
                f"article {article.get('candidate_id')} summary safety_flags are not false"
            )

    audit = {
        "schema_version": "real-corpus-smoke-audit.v1",
        "manifest_ref": f"artifact:{manifest_path.as_posix()}",
        "run_summary_ref": f"artifact:{(run_dir / 'summary.json').as_posix()}",
        "article_count": len(manifest_articles),
        "completed_handoff_count": int(run_summary.get("completed_handoff_count") or 0),
        "ready_queue_count": ready_count,
        "coverage": {
            "articles_with_source_refs": articles_with_source_refs,
            "articles_with_loader_refs": articles_with_loader_refs,
            "articles_with_explicit_loader_absence": articles_with_explicit_loader_absence,
            "articles_missing_loader_refs": len(run_articles) - articles_with_loader_refs,
            "continuity_artifacts_present": len(continuity_records),
            "per_article_artifact_sets_complete": missing_artifacts == 0,
        },
        "diagnostics": dict(sorted(diagnostics.items())),
        "safety": safety,
        "artifact_safety": {
            "missing_artifact_count": missing_artifacts,
            "unsafe_payload_file_count": unsafe_payload_files,
        },
        "article_artifacts": article_artifact_status,
        "blockers_for_import": blockers_from(
            diagnostics,
            missing_artifacts=missing_artifacts,
            unsafe_payload_files=unsafe_payload_files,
            missing_continuity_artifacts=missing_continuity_artifacts,
        ),
        "next_safe_step": "Run a larger no-write real-corpus batch or add a GraphDB comparison milestone only after continuity blockers are resolved by explicit ADR gates.",
    }
    return audit


def write_markdown_report(audit: dict[str, Any], output_path: Path) -> None:
    diagnostics = audit.get("diagnostics", {})
    blockers = audit.get("blockers_for_import", [])
    coverage = audit.get("coverage", {})
    safety = audit.get("safety", {})
    lines = [
        "# M036 Real Corpus No Write Smoke Audit",
        "",
        f"- Articles: {audit['article_count']}",
        f"- Completed handoffs: {audit['completed_handoff_count']}",
        f"- Ready queue jobs: {audit['ready_queue_count']}",
        f"- Source refs present: {coverage.get('articles_with_source_refs')}/{audit['article_count']}",
        f"- Loader refs present: {coverage.get('articles_with_loader_refs')}/{audit['article_count']}",
        f"- Loader absence explicit: {coverage.get('articles_with_explicit_loader_absence')}/{audit['article_count']}",
        f"- Continuity artifacts present: {coverage.get('continuity_artifacts_present')}/{audit['article_count']}",
        f"- Artifact sets complete: {str(coverage.get('per_article_artifact_sets_complete')).lower()}",
        "",
        "## Safety",
        "",
        f"- GraphDB write: {str(safety.get('graph_write_allowed')).lower()}",
        f"- Promotion: {str(safety.get('promotion_allowed')).lower()}",
        f"- Production import: {str(safety.get('production_import_attempted')).lower()}",
        f"- Import eligible: {str(safety.get('import_eligible')).lower()}",
        "",
        "## Diagnostics",
        "",
    ]
    if diagnostics:
        lines.extend(f"- {key}: {value}" for key, value in sorted(diagnostics.items()))
    else:
        lines.append("- none")
    lines.extend(["", "## Blockers for Import", ""])
    if blockers:
        lines.extend(f"- {blocker}" for blocker in blockers)
    else:
        lines.append("- none for no-write smoke scope")
    lines.extend(["", "## Next safe step", "", str(audit["next_safe_step"]), ""])
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--output-json", type=Path, required=True)
    parser.add_argument("--output-md", type=Path, required=True)
    args = parser.parse_args()

    audit = audit_smoke(args.manifest, args.run_dir)
    write_json(args.output_json, audit)
    write_markdown_report(audit, args.output_md)
    emit(f"article_count={audit['article_count']}")
    emit(f"completed_handoff_count={audit['completed_handoff_count']}")
    emit(f"blockers_for_import={','.join(audit['blockers_for_import']) or 'none'}")
    emit(
        "graph_write_allowed=false promotion_allowed=false production_import_attempted=false import_eligible=false"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
