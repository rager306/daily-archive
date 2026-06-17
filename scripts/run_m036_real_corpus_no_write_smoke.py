#!/usr/bin/env python3
"""Run M036 real-corpus metadata-only no-write smoke."""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

from arxiv_archive.minimax_structured import DEFAULT_MINIMAX_MODEL  # noqa: E402
from research_graph.workflows.universal_kb.contracts import CandidatePacket  # noqa: E402
from research_graph.workflows.universal_kb.queue import UniversalKBQueue  # noqa: E402
from research_graph.workflows.universal_kb.review_assistance import (  # noqa: E402
    build_review_assistance_packet,
    build_review_tool_invocation_record,
)
from research_graph.workflows.universal_kb.substrate_rehearsal import NoWriteSubstrateRehearsal  # noqa: E402

FALSE_SAFETY_KEYS = ("graph_write_allowed", "promotion_allowed", "production_import_attempted", "import_eligible")
CONTINUITY_SCHEMA_VERSION = "m040-real-corpus-continuity.v1"
LEGACY_SAFETY_DIAGNOSTIC = "safety_flags_missing_or_not_false"
LEGACY_LOADER_DIAGNOSTIC = "missing_loader_evidence"
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


def assert_false_flags(flags: dict[str, Any], *, label: str) -> None:
    for key in FALSE_SAFETY_KEYS:
        if flags.get(key) is not False:
            raise ValueError(f"{label} safety flag {key} must be false")


def safe_slug(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in {"-", "_"} else "_" for ch in value)[:120]


def require_output_inside_manifest_dir(manifest_path: Path, output_dir: Path) -> None:
    manifest_root = manifest_path.resolve().parent
    resolved_output = output_dir.resolve()
    if resolved_output != manifest_root and manifest_root not in resolved_output.parents:
        raise ValueError("output_dir must be inside the manifest directory when --clean is used")


def clean_output(output_dir: Path) -> None:
    if output_dir.exists():
        shutil.rmtree(output_dir)


def assert_artifact_payload_safe(output_dir: Path) -> None:
    for path in output_dir.rglob("*.json"):
        text = path.read_text(encoding="utf-8", errors="ignore").lower()
        for term in FORBIDDEN_PAYLOAD_TERMS:
            if term in text:
                raise ValueError(f"{path} contains forbidden payload term {term!r}")


def build_candidate(article: dict[str, Any]) -> CandidatePacket:
    return CandidatePacket(
        candidate_id=str(article["candidate_id"]),
        candidate_type=str(article.get("candidate_type") or "real_article_metadata"),
        evidence_refs=tuple(str(ref) for ref in article.get("evidence_refs", ())),
    )


def source_refs(evidence_refs: tuple[str, ...]) -> list[str]:
    return [ref for ref in evidence_refs if "/source/" in ref or ref.endswith("/article.json")]


def loader_refs(evidence_refs: tuple[str, ...]) -> list[str]:
    return [ref for ref in evidence_refs if "/loader/" in ref]


def normalized_diagnostics(article: dict[str, Any], *, has_loader_refs: bool) -> list[str]:
    legacy_diagnostics = {LEGACY_SAFETY_DIAGNOSTIC, LEGACY_LOADER_DIAGNOSTIC}
    diagnostics = [
        str(value)
        for value in article.get("diagnostics", ())
        if str(value).strip() and str(value) not in legacy_diagnostics
    ]
    diagnostics.append("loader_evidence_present" if has_loader_refs else "loader_evidence_absent_explicit")
    diagnostics.append("article_safety_flags_explicit_false")
    return diagnostics


def build_continuity_metadata(
    article: dict[str, Any],
    *,
    candidate: CandidatePacket,
    handoff_payload: dict[str, Any],
) -> dict[str, Any]:
    evidence_refs = tuple(str(ref) for ref in candidate.evidence_refs)
    article_loader_refs = loader_refs(evidence_refs)
    article_source_refs = source_refs(evidence_refs)
    safety_flags = {
        "graph_write_allowed": handoff_payload["graph_write_allowed"],
        "promotion_allowed": handoff_payload["promotion_allowed"],
        "production_import_attempted": handoff_payload["production_import_attempted"],
        "import_eligible": False,
    }
    assert_false_flags(safety_flags, label=f"{candidate.candidate_id} continuity")
    return {
        "schema_version": CONTINUITY_SCHEMA_VERSION,
        "article_key": article.get("article_key"),
        "candidate_id": candidate.candidate_id,
        "candidate_type": candidate.candidate_type,
        "safety_flags": safety_flags,
        "source_evidence": {
            "status": "present" if article_source_refs else "absent",
            "refs": article_source_refs,
            "ref_count": len(article_source_refs),
        },
        "loader_evidence": {
            "status": "present" if article_loader_refs else "absent_explicit",
            "refs": article_loader_refs,
            "ref_count": len(article_loader_refs),
            "diagnostic": None if article_loader_refs else "loader_evidence_absent_explicit",
        },
        "diagnostics": normalized_diagnostics(article, has_loader_refs=bool(article_loader_refs)),
        "import_eligibility": {
            "import_eligible": False,
            "reason": "real_corpus_no_write_smoke_continuity_only",
        },
        "metadata_only": True,
    }


def run_article(article: dict[str, Any], *, output_dir: Path) -> dict[str, Any]:
    article_flags = article.get("safety_flags") if isinstance(article.get("safety_flags"), dict) else {}
    assert_false_flags(article_flags, label=str(article.get("article_key") or article.get("candidate_id")))
    candidate = build_candidate(article)
    diagnostics = tuple(str(value) for value in article.get("diagnostics", ()) if str(value).strip())
    if not diagnostics:
        diagnostics = ("real_article_metadata_smoke",)
    review_packet = build_review_assistance_packet(
        candidate=candidate,
        diagnostics=diagnostics,
        confidence=0.55,
        flags=("needs_human_review", "real_corpus_smoke"),
    )
    review_trace = build_review_tool_invocation_record(
        invocation_id=f"m036-review:{candidate.candidate_id}",
        model=DEFAULT_MINIMAX_MODEL,
        input_hash="sha256:m036-real-corpus-smoke-metadata",
        review_packet=review_packet,
    )

    article_dir = output_dir / "articles" / safe_slug(candidate.candidate_id)
    article_dir.mkdir(parents=True, exist_ok=True)
    queue = UniversalKBQueue(article_dir / "queue.sqlite").initialize()
    try:
        queue.enqueue(
            job_id=candidate.candidate_id,
            stage="real_corpus_review_assistance",
            input_refs=candidate.evidence_refs,
            input_hash="sha256:m036-real-corpus-smoke-metadata",
            tool_version=DEFAULT_MINIMAX_MODEL,
            contract_version=review_packet.schema_version,
            output_paths=("readiness_handoff.json",),
        )
        queue.unblock_ready_jobs()
        queue_inspect = queue.inspect(candidate.candidate_id)
        handoff = NoWriteSubstrateRehearsal(queue).build_handoff(
            candidate=candidate,
            review_trace=review_trace,
            queue_job_id=candidate.candidate_id,
        )
    finally:
        queue.close()
        for suffix in ("", "-wal", "-shm"):
            path = article_dir / f"queue.sqlite{suffix}"
            if path.exists():
                path.unlink()

    write_json(article_dir / "candidate.json", candidate.to_dict())
    write_json(article_dir / "review_packet.json", review_packet.to_dict())
    write_json(article_dir / "review_trace.json", review_trace.to_sanitized_dict())
    write_json(article_dir / "queue_inspect.json", queue_inspect)
    handoff_payload = handoff.to_dict()
    write_json(article_dir / "readiness_handoff.json", handoff_payload)
    continuity = build_continuity_metadata(article, candidate=candidate, handoff_payload=handoff_payload)
    write_json(article_dir / "continuity.json", continuity)

    return {
        "article_key": article.get("article_key"),
        "candidate_id": candidate.candidate_id,
        "queue_status": queue_inspect["job"]["status"],
        "diagnostics": continuity["diagnostics"],
        "artifact_dir": str(article_dir),
        "continuity_ref": f"artifact:{(article_dir / 'continuity.json').as_posix()}",
        "source_ref_count": continuity["source_evidence"]["ref_count"],
        "loader_ref_count": continuity["loader_evidence"]["ref_count"],
        "loader_evidence_status": continuity["loader_evidence"]["status"],
        "safety_flags": continuity["safety_flags"],
        "graph_write_allowed": handoff_payload["graph_write_allowed"],
        "promotion_allowed": handoff_payload["promotion_allowed"],
        "production_import_attempted": handoff_payload["production_import_attempted"],
        "import_eligible": False,
    }


def run_smoke(manifest_path: Path, *, output_dir: Path, clean: bool = False) -> dict[str, Any]:
    manifest = load_json(manifest_path)
    flags = manifest.get("safety_flags") if isinstance(manifest.get("safety_flags"), dict) else {}
    assert_false_flags(flags, label="manifest")
    articles = manifest.get("articles")
    if not isinstance(articles, list) or not articles:
        raise ValueError("manifest must contain articles")
    if clean:
        require_output_inside_manifest_dir(manifest_path, output_dir)
        clean_output(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    article_summaries = [run_article(article, output_dir=output_dir) for article in articles]
    summary = {
        "schema_version": "m036-real-corpus-no-write-smoke-summary.v1",
        "manifest_ref": f"artifact:{manifest_path.as_posix()}",
        "article_count": len(article_summaries),
        "completed_handoff_count": sum(1 for item in article_summaries if item["queue_status"] == "ready"),
        "model": DEFAULT_MINIMAX_MODEL,
        "articles": article_summaries,
        "diagnostics": manifest.get("diagnostics", []),
        "graph_write_allowed": False,
        "promotion_allowed": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }
    write_json(output_dir / "summary.json", summary)
    assert_artifact_payload_safe(output_dir)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--clean", action="store_true")
    args = parser.parse_args()

    summary = run_smoke(args.manifest, output_dir=args.output_dir, clean=args.clean)
    emit(f"article_count={summary['article_count']}")
    emit(f"completed_handoff_count={summary['completed_handoff_count']}")
    emit(f"output={args.output_dir}")
    emit("graph_write_allowed=false promotion_allowed=false production_import_attempted=false import_eligible=false")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
