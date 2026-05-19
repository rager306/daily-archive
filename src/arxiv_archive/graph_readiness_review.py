"""Bounded artifact review bundle generation for graph-readiness packages."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from arxiv_archive.full_text import FullTextSource, ingest_full_text
from arxiv_archive.graph_readiness import (
    GraphReadinessState,
    NormalizedPaperPackage,
    to_redacted_dict,
)
from arxiv_archive.graph_readiness_export import build_package_from_manifest_document

REQUIRED_REPAIRED_PAPERS = ("2605.14259v1", "2605.14517v1")
DEFAULT_SNIPPET_CHARS = 700
EVENTS_FILENAME = "independent-review-events.jsonl"
SUMMARY_FILENAME = "independent-review-summary.md"


@dataclass(frozen=True)
class ReviewBundleResult:
    """Review artifact generation result."""

    selected_paper_ids: list[str]
    review_paths: list[Path]
    summary_path: Path
    events_path: Path


@dataclass(frozen=True)
class ReviewArtifactValidation:
    """Validation result for generated or completed review artifacts."""

    ok: bool
    diagnostics: list[str]


def generate_review_bundles(
    *,
    corpus_path: Path,
    review_dir: Path,
    events_path: Path,
    run_id: str | None = None,
    required_paper_ids: tuple[str, ...] = REQUIRED_REPAIRED_PAPERS,
    snippet_chars: int = DEFAULT_SNIPPET_CHARS,
) -> ReviewBundleResult:
    """Generate bounded markdown review bundles and structured review request events."""
    run_id = run_id or _default_run_id()
    corpus = json.loads(Path(corpus_path).read_text(encoding="utf-8"))
    docs = list(corpus.get("documents", []))
    packages = [build_package_from_manifest_document(doc, run_id=run_id) for doc in docs]
    docs_by_id = {str(doc["paper_id"]): doc for doc in docs}
    packages_by_id = {package.paper_id: package for package in packages}
    selected = select_review_papers(packages, required_paper_ids=required_paper_ids)

    review_dir = Path(review_dir)
    review_dir.mkdir(parents=True, exist_ok=True)
    events_path = Path(events_path)
    events_path.parent.mkdir(parents=True, exist_ok=True)
    if events_path.exists():
        events_path.unlink()

    review_paths: list[Path] = []
    for paper_id in selected:
        package = packages_by_id[paper_id]
        doc = docs_by_id[paper_id]
        path = review_dir / f"{paper_id}-review.md"
        path.write_text(
            render_review_bundle(package, doc, snippet_chars=snippet_chars),
            encoding="utf-8",
        )
        review_paths.append(path)
        _append_event(events_path, _review_request_event(run_id, package, path))

    summary_path = review_dir / SUMMARY_FILENAME
    summary_path.write_text(
        render_review_summary(selected, review_paths, events_path),
        encoding="utf-8",
    )
    _append_event(
        events_path,
        {
            "event": "independent_review.summary",
            "run_id": run_id,
            "selected_paper_ids": selected,
            "review_count": len(review_paths),
            "summary_path": str(summary_path),
        },
    )
    return ReviewBundleResult(
        selected_paper_ids=selected,
        review_paths=review_paths,
        summary_path=summary_path,
        events_path=events_path,
    )


def select_review_papers(
    packages: list[NormalizedPaperPackage],
    *,
    required_paper_ids: tuple[str, ...] = REQUIRED_REPAIRED_PAPERS,
) -> list[str]:
    """Select required repaired, baseline-good, complex, and blocker papers."""
    by_id = {package.paper_id: package for package in packages}
    selected: list[str] = []

    for paper_id in required_paper_ids:
        if paper_id in by_id:
            selected.append(paper_id)

    originally_good = next(
        (
            package.paper_id
            for package in packages
            if package.paper_id not in selected
            and package.report.state == GraphReadinessState.OK_FOR_GRAPH
            and package.report.counts.get("chunks", 0) > 0
        ),
        None,
    )
    if originally_good is not None:
        selected.append(originally_good)

    complex_candidate = _select_complex_candidate(packages, already_selected=set(selected))
    if complex_candidate is not None:
        selected.append(complex_candidate)

    for package in packages:
        if package.paper_id in selected:
            continue
        if package.report.state in {GraphReadinessState.REPAIR_REQUIRED, GraphReadinessState.REJECT}:
            selected.append(package.paper_id)

    return selected


def render_review_bundle(
    package: NormalizedPaperPackage,
    manifest_doc: dict[str, Any],
    *,
    snippet_chars: int = DEFAULT_SNIPPET_CHARS,
) -> str:
    """Render one bounded paper review bundle as Markdown."""
    source_path = Path(manifest_doc.get("expected_full_text_path") or Path(manifest_doc["paper_dir"]) / "full_text.md")
    source = FullTextSource(paper_id=package.paper_id, source_type="markdown", source_path=source_path)
    ingestion = ingest_full_text(source)
    lines = [
        f"# Independent Review Bundle — {package.paper_id}",
        "",
        "## Claim Under Review",
        "",
        "This paper's current graph-readiness package is structurally ready for route eligibility review. The reviewer must decide whether the chunks and routes are semantically graph-ready or require flags/repairs/exclusions.",
        "",
        "## Paper Metadata",
        "",
        f"- paper_id: `{package.paper_id}`",
        f"- title: {manifest_doc.get('title', 'unknown')}",
        f"- source_path: `{source_path}`",
        f"- state: `{package.report.state.value}`",
        f"- trust_level: `{package.report.trust_level.value}`",
        f"- chunks: {package.report.counts.get('chunks', 0)}",
        f"- evidence_paths: {package.report.counts.get('evidence_paths', 0)}",
        "",
        "## Route Summary",
        "",
        "| Route | Eligible | Blocked |",
        "|---|---:|---:|",
    ]
    for route, details in sorted(package.report.routes.items()):
        lines.append(f"| `{route}` | {details.get('eligible', 0)} | {details.get('blocked', 0)} |")

    lines.extend([
        "",
        "## Warnings",
        "",
    ])
    if package.warnings:
        for warning in package.warnings:
            lines.append(f"- `{warning.severity.value}` `{warning.code}` — {warning.message}")
    else:
        lines.append("- None recorded by automated baseline.")

    lines.extend([
        "",
        "## Chunk Samples",
        "",
    ])
    for chunk in _sample_chunks(package):
        snippet = _snippet_for_chunk(ingestion.text, chunk.source_span.char_start, chunk.source_span.char_end, snippet_chars)
        lines.extend(
            [
                f"### {chunk.chunk_id}",
                "",
                f"- candidate_id: `{chunk.chunk_id}`",
                f"- parent_chunk_id: `{chunk.parent_chunk_id or 'none'}`",
                f"- type: `{chunk.chunk_type.value}`",
                f"- routes: {', '.join(f'`{route.value}`' for route in chunk.routes)}",
                f"- excluded_routes: {', '.join(f'`{route.value}`' for route in chunk.excluded_routes) or 'none'}",
                f"- section_path: {' / '.join(chunk.section_path)}",
                f"- char_count: {chunk.char_count}",
                f"- source_span: `{chunk.source_span.coordinate_space.value}:{chunk.source_span.char_start}-{chunk.source_span.char_end}`",
                f"- quality_state: `{chunk.quality_state.value}`",
                f"- warnings: {_chunk_warning_summary(chunk)}",
                "",
                "```text",
                snippet,
                "```",
                "",
            ]
        )

    lines.extend(
        [
            "## Reviewer Checklist",
            "",
            "- Does the converted text clearly belong to the intended paper?",
            "- Are chunks semantically coherent and reasonably atomic for their routes?",
            "- Are references/metadata/table/figure chunks routed away from prose claim extraction when appropriate?",
            "- Are source spans sufficient to trace claims back to the artifact?",
            "- Are any route claims count-only or schema-only false confidence?",
            "",
            "## Reviewer Output Contract",
            "",
            "Return a completed review result; do not leave placeholders or repeat the checklist.",
            "Use this exact structure so the orchestrator can copy it into summary Markdown and JSONL events:",
            "",
            "```yaml",
            "verdict: PASS | FLAG | REPAIR | BLOCKER",
            "safe_to_promote:",
            "  - paper_id: <paper_id>",
            "    route: <route>",
            "    eligibility: eligible | eligible_with_caveat",
            "    evidence: <chunk ids and why they are coherent/source-traceable>",
            "caveats:",
            "  - <route-specific caveat that must appear in manifest>",
            "repair_required:",
            "  - paper_id: <paper_id>",
            "    chunk_id: <chunk_id or route>",
            "    candidate_id: <candidate chunk_id when reviewing a specific candidate, otherwise null>",
            "    route: <route>",
            "    finding_code: <stable_snake_case_code>",
            "    finding: <what is semantically wrong>",
            "    required_action: <repair or exclusion needed before eligibility>",
            "route_exclusions:",
            "  - paper_id: <paper_id or '*'>",
            "    route: <route>",
            "    reason: <why it must not be promoted>",
            "manifest_implications:",
            "  - <exact final_eligibility / independent_review_verdict instruction for T04>",
            "```",
            "",
            "If a field has no items, return an empty list (`[]`). If evidence is sample-scoped, say so explicitly.",
        ]
    )
    return "\n".join(lines)


def render_review_summary(
    selected_paper_ids: list[str],
    review_paths: list[Path],
    events_path: Path,
) -> str:
    """Render a summary index for generated review bundles."""
    lines = [
        "# Independent Review Summary — Graph Readiness",
        "",
        "## Status",
        "",
        "Review bundles generated. Independent reviewer verdicts are still required before route eligibility can be claimed.",
        "",
        "## Selected Papers",
        "",
    ]
    for paper_id, path in zip(selected_paper_ids, review_paths, strict=True):
        lines.append(f"- `{paper_id}` — `{path}`")
    lines.extend(
        [
            "",
            "## Review Events",
            "",
            f"- `{events_path}`",
            "",
            "## Required Next Step",
            "",
            "Run independent review using `.gsd/milestones/M004-ubh2pt/slices/S11/independent-review-gate.md`. The reviewer must return the completed `Reviewer Output Contract` block from each bundle; do not accept unreplaced placeholders, checklist echoes, or count-only verdicts. Record PASS/FLAG/REPAIR/BLOCKER findings before creating an eligibility manifest.",
        ]
    )
    return "\n".join(lines)


def validate_review_artifacts(
    *,
    review_dir: Path,
    events_path: Path,
    require_completed_review: bool = False,
) -> ReviewArtifactValidation:
    """Validate that review artifacts have no stale placeholders and, optionally, a completed verdict."""
    diagnostics: list[str] = []
    review_dir = Path(review_dir)
    events_path = Path(events_path)
    review_paths = sorted(review_dir.glob("*-review.md"))
    if not review_paths:
        diagnostics.append(f"No review bundle files found in {review_dir}")

    forbidden_phrases = (
        "Reviewer Verdict Placeholder",
        "Verdict: PASS / FLAG / REPAIR / BLOCKER",
        "Findings:\n- ",
        "<paper_id>",
        "<route>",
        "<chunk_id or route>",
        "<candidate chunk_id when reviewing a specific candidate, otherwise null>",
        "<stable_snake_case_code>",
    )
    for path in review_paths:
        text = path.read_text(encoding="utf-8")
        if "Reviewer Output Contract" not in text:
            diagnostics.append(f"{path} is missing Reviewer Output Contract")
        for phrase in forbidden_phrases[:3]:
            if phrase in text:
                diagnostics.append(f"{path} contains stale placeholder phrase: {phrase}")

    events = _read_events(events_path, diagnostics)
    verdict_events = [event for event in events if event.get("event") == "independent_review.verdict"]
    if require_completed_review:
        summary_path = review_dir / SUMMARY_FILENAME
        if not summary_path.exists():
            diagnostics.append(f"Missing review summary: {summary_path}")
        else:
            summary_text = summary_path.read_text(encoding="utf-8")
            for phrase in forbidden_phrases:
                if phrase in summary_text:
                    diagnostics.append(f"{summary_path} contains unreplaced placeholder phrase: {phrase}")
            if "verdict: PASS | FLAG | REPAIR | BLOCKER" in summary_text:
                diagnostics.append(f"{summary_path} contains an unfilled output contract verdict union")
        if not verdict_events:
            diagnostics.append("No independent_review.verdict event found in completed review events")
        for event in verdict_events:
            verdict = str(event.get("verdict", ""))
            if verdict not in {"PASS", "FLAG", "REPAIR", "BLOCKER"}:
                diagnostics.append(f"Invalid completed review verdict: {verdict}")
            if event.get("output_contract_completed") is not True:
                diagnostics.append("Completed review verdict is missing output_contract_completed=true")
    return ReviewArtifactValidation(ok=not diagnostics, diagnostics=diagnostics)


def _read_events(path: Path, diagnostics: list[str]) -> list[dict[str, Any]]:
    if not path.exists():
        diagnostics.append(f"Missing review events file: {path}")
        return []
    events: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError as exc:
            diagnostics.append(f"{path}:{line_number} invalid JSONL: {exc}")
    return events


def _select_complex_candidate(
    packages: list[NormalizedPaperPackage],
    *,
    already_selected: set[str],
) -> str | None:
    candidates: list[tuple[int, str]] = []
    for package in packages:
        if package.paper_id in already_selected:
            continue
        score = 0
        for route in ("table_extraction", "figure_evidence", "citation_graph"):
            details = package.report.routes.get(route, {})
            score += int(details.get("eligible", 0)) + int(details.get("blocked", 0))
        for chunk in package.chunks:
            score += sum(
                1
                for route in chunk.excluded_routes
                if route.value in {"table_extraction", "figure_evidence", "citation_graph"}
            )
        if score > 0:
            candidates.append((score, package.paper_id))
    if candidates:
        return sorted(candidates, reverse=True)[0][1]
    return None


def _sample_chunks(package: NormalizedPaperPackage) -> list[Any]:
    selected = []
    preferred_routes = [
        "claim_extraction",
        "method_extraction",
        "table_extraction",
        "figure_evidence",
        "citation_graph",
        "metadata_graph",
        "retrieval_only",
    ]
    seen_ids: set[str] = set()
    atomic_split_chunks = [
        chunk
        for chunk in package.chunks
        if any(warning.code == "atomic_claim_candidate_split" for warning in chunk.validation_warnings)
    ]
    repair_candidate_chunks = [
        chunk
        for chunk in package.chunks
        if any(warning.code == "multi_claim_candidate_requires_atomic_split" for warning in chunk.validation_warnings)
    ]
    split_chunks = [chunk for chunk in package.chunks if chunk.parent_chunk_id or ":split-" in chunk.chunk_id]
    for chunk in [*atomic_split_chunks[:6], *repair_candidate_chunks[:3], *split_chunks[:4]]:
        if chunk.chunk_id in seen_ids:
            continue
        selected.append(chunk)
        seen_ids.add(chunk.chunk_id)
    for route_name in preferred_routes:
        for chunk in [*split_chunks, *package.chunks]:
            if chunk.chunk_id in seen_ids:
                continue
            if any(route.value == route_name for route in chunk.routes):
                selected.append(chunk)
                seen_ids.add(chunk.chunk_id)
                break
    for chunk in package.chunks:
        if len(selected) >= 10:
            break
        if chunk.chunk_id not in seen_ids:
            selected.append(chunk)
            seen_ids.add(chunk.chunk_id)
    return selected


def _chunk_warning_summary(chunk: Any) -> str:
    warnings = list(getattr(chunk, "validation_warnings", []))
    if not warnings:
        return "none"
    return "; ".join(
        f"`{warning.severity.value}` `{warning.code}` — {warning.message}"
        for warning in warnings[:5]
    )


def _snippet_for_chunk(text: str, start: int | None, end: int | None, limit: int) -> str:
    if start is None or end is None or start < 0 or end <= start:
        return "[unavailable: missing source span]"
    snippet = text[start:end].strip().replace("\x00", "")
    if len(snippet) <= limit:
        return snippet
    return snippet[:limit].rstrip() + " …"


def _review_request_event(run_id: str, package: NormalizedPaperPackage, path: Path) -> dict[str, Any]:
    return {
        "event": "independent_review.requested",
        "run_id": run_id,
        "paper_id": package.paper_id,
        "state": package.report.state,
        "trust_level": package.report.trust_level,
        "review_artifact_path": str(path),
        "routes": package.report.routes,
        "raw_text_included": True,
        "raw_text_scope": "bounded_chunk_snippets",
    }


def _append_event(path: Path, payload: dict[str, Any]) -> None:
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(to_redacted_dict(payload), sort_keys=True) + "\n")


def _default_run_id() -> str:
    return f"independent-review-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate or validate graph-readiness review bundles.")
    parser.add_argument("--corpus", type=Path)
    parser.add_argument("--review-dir", required=True, type=Path)
    parser.add_argument("--events", required=True, type=Path)
    parser.add_argument("--run-id", default=None)
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Validate existing review artifacts instead of generating bundles.",
    )
    parser.add_argument(
        "--require-completed-review",
        action="store_true",
        help="Require a completed independent_review.verdict event and no placeholders in the summary.",
    )
    args = parser.parse_args(argv)
    if args.validate_only:
        validation = validate_review_artifacts(
            review_dir=args.review_dir,
            events_path=args.events,
            require_completed_review=args.require_completed_review,
        )
        sys.stdout.write(json.dumps({"ok": validation.ok, "diagnostics": validation.diagnostics}, indent=2, sort_keys=True))
        sys.stdout.write("\n")
        return 0 if validation.ok else 1
    if args.corpus is None:
        parser.error("--corpus is required unless --validate-only is set")
    result = generate_review_bundles(
        corpus_path=args.corpus,
        review_dir=args.review_dir,
        events_path=args.events,
        run_id=args.run_id,
    )
    sys.stdout.write(
        json.dumps(
            {
                "selected_paper_ids": result.selected_paper_ids,
                "review_paths": [str(path) for path in result.review_paths],
                "summary_path": str(result.summary_path),
                "events_path": str(result.events_path),
            },
            indent=2,
            sort_keys=True,
        )
    )
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
