#!/usr/bin/env python3
"""Continuity audit for the M036 real-corpus no-write smoke artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

FALSE_SAFETY_KEYS = ("graph_write_allowed", "promotion_allowed", "production_import_attempted", "import_eligible")
PER_ARTICLE_ARTIFACTS = (
    "candidate.json",
    "review_packet.json",
    "review_trace.json",
    "queue_inspect.json",
    "readiness_handoff.json",
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


def inspect_article_artifacts(article: dict[str, Any]) -> dict[str, Any]:
    article_dir = Path(str(article["artifact_dir"]))
    missing = [name for name in PER_ARTICLE_ARTIFACTS if not (article_dir / name).exists()]
    unsafe_payload_files = [
        str(path)
        for path in article_dir.glob("*.json")
        if path.is_file() and not artifact_payload_safe(path)
    ]
    return {
        "artifact_dir": str(article_dir),
        "missing_artifacts": missing,
        "unsafe_payload_files": unsafe_payload_files,
    }


def blockers_from(diagnostics: Counter[str], missing_artifacts: int, unsafe_payload_files: int) -> list[str]:
    blockers: list[str] = []
    if diagnostics.get("missing_loader_evidence", 0):
        blockers.append("missing_loader_evidence")
    if diagnostics.get("safety_flags_missing_or_not_false", 0):
        blockers.append("legacy_or_missing_article_safety_flags")
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

    diagnostics = Counter(
        diagnostic
        for article in manifest_articles
        for diagnostic in article.get("diagnostics", [])
    )
    article_artifact_status = [inspect_article_artifacts(article) for article in run_articles]
    missing_artifacts = sum(len(item["missing_artifacts"]) for item in article_artifact_status)
    unsafe_payload_files = sum(len(item["unsafe_payload_files"]) for item in article_artifact_status)
    articles_with_source_refs = sum(1 for article in manifest_articles if int(article.get("source_file_count") or 0) > 0)
    articles_with_loader_refs = sum(1 for article in manifest_articles if int(article.get("loader_ref_count") or 0) > 0)
    ready_count = sum(1 for article in run_articles if article.get("queue_status") == "ready")

    safety = safety_summary(run_summary)
    if any(safety.values()):
        raise ValueError("run summary contains true write/import/promotion flag")
    for article in run_articles:
        if not false_flags(article):
            raise ValueError(f"article {article.get('candidate_id')} contains true write/import/promotion flag")

    audit = {
        "schema_version": "m036-real-corpus-smoke-audit.v1",
        "manifest_ref": f"artifact:{manifest_path.as_posix()}",
        "run_summary_ref": f"artifact:{(run_dir / 'summary.json').as_posix()}",
        "article_count": len(manifest_articles),
        "completed_handoff_count": int(run_summary.get("completed_handoff_count") or 0),
        "ready_queue_count": ready_count,
        "coverage": {
            "articles_with_source_refs": articles_with_source_refs,
            "articles_with_loader_refs": articles_with_loader_refs,
            "articles_missing_loader_refs": len(manifest_articles) - articles_with_loader_refs,
            "per_article_artifact_sets_complete": missing_artifacts == 0,
        },
        "diagnostics": dict(sorted(diagnostics.items())),
        "safety": safety,
        "artifact_safety": {
            "missing_artifact_count": missing_artifacts,
            "unsafe_payload_file_count": unsafe_payload_files,
        },
        "article_artifacts": article_artifact_status,
        "blockers_for_import": blockers_from(diagnostics, missing_artifacts, unsafe_payload_files),
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
    emit("graph_write_allowed=false promotion_allowed=false production_import_attempted=false import_eligible=false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
