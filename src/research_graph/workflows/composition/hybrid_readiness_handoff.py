"""M216/M218/M227: hybrid coverage + readiness handoff (+ scholarly wrapper).

Orchestrates:
1) hybrid selection → catalog coverage (M215)
2) resolve hybrid body markdown paths under a body_root
3) resolve GROBID hybrid.header.json / hybrid.citations.jsonl (M217/M218)
4) no-write graph-data readiness on available bodies (M209)
5) M227: scholarly preprocess summary per found body (enrichment only)

Never authorizes import/writes. Does not start GROBID/ODL (artifacts must pre-exist).
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from research_graph.application.corpus.language_detect import detect_text_language
from research_graph.application.corpus.preprocess_rollup import (
    rollup_preprocess_bodies,
)
from research_graph.application.corpus.preprocess_summary import (
    preprocess_summary_for_body,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.workflows.composition.graph_data_readiness import (
    GraphDataReadinessRequest,
    GraphDataReadinessResult,
    SourceInput,
    run_graph_data_readiness_pipeline,
)
from research_graph.workflows.composition.hybrid_catalog_coverage import (
    HybridCatalogCoverageRequest,
    HybridCatalogCoverageResult,
    run_hybrid_catalog_coverage,
)
from research_graph.workflows.composition.yake_keyword_inject import (
    cleaned_body_for_yake,
    yake_keywords_for_text,
    yake_language_code,
)

DEFAULT_SELECTION = Path("artifacts/m213-hybrid-gate/selection-20.json")
DEFAULT_BODY_ROOT = Path("artifacts/m213-hybrid-gate/runs-live-20")
DEFAULT_CATALOG_INDEX = Path("data/article_catalog/index.json")
DEFAULT_CATALOG_ROOT = Path("data/article_catalog")
SCHEMA_VERSION = "m227-hybrid-readiness-handoff.v1"

HandoffVerdict = Literal["ready_for_review", "repair", "blocked"]


@dataclass(frozen=True, slots=True)
class BodyPathResolution:
    paper_id: str
    pdf_path: str
    body_path: str | None
    found: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "pdf_path": self.pdf_path,
            "body_path": self.body_path,
            "found": self.found,
        }


@dataclass(frozen=True, slots=True)
class ScholarlyArtifactResolution:
    """Per-paper GROBID header/citations artifact presence (candidate-only)."""

    paper_id: str
    header_path: str | None
    header_found: bool
    citations_path: str | None
    citations_found: bool
    citation_count: int
    header_title: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "header_path": self.header_path,
            "header_found": self.header_found,
            "citations_path": self.citations_path,
            "citations_found": self.citations_found,
            "citation_count": self.citation_count,
            "header_title": self.header_title,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


def resolve_scholarly_artifact_paths(
    hybrid_selection: dict[str, Any],
    *,
    body_root: Path,
    load_counts: bool = True,
) -> tuple[ScholarlyArtifactResolution, ...]:
    """Resolve `{body_root}/{paper_id}/body/{paper_id}.hybrid.{header.json,citations.jsonl}`.

    Optional load_counts reads citation jsonl line count and header title.
    Never invents missing files; never authorizes import.
    """
    papers = hybrid_selection.get("papers")
    if not isinstance(papers, list):
        return ()
    rows: list[ScholarlyArtifactResolution] = []
    for raw in papers:
        if not isinstance(raw, dict):
            continue
        paper_id = str(raw.get("paper_id") or "").strip()
        if not paper_id:
            rows.append(
                ScholarlyArtifactResolution(
                    paper_id="",
                    header_path=None,
                    header_found=False,
                    citations_path=None,
                    citations_found=False,
                    citation_count=0,
                )
            )
            continue
        body_dir = body_root / paper_id / "body"
        header_p = body_dir / f"{paper_id}.hybrid.header.json"
        cites_p = body_dir / f"{paper_id}.hybrid.citations.jsonl"
        header_found = header_p.is_file()
        cites_found = cites_p.is_file()
        citation_count = 0
        header_title: str | None = None
        if load_counts and header_found:
            try:
                header_obj = json.loads(header_p.read_text(encoding="utf-8"))
                if isinstance(header_obj, dict):
                    title = header_obj.get("title")
                    header_title = str(title) if title else None
            except (OSError, json.JSONDecodeError):
                header_title = None
        if load_counts and cites_found:
            try:
                citation_count = sum(
                    1
                    for line in cites_p.read_text(encoding="utf-8").splitlines()
                    if line.strip()
                )
            except OSError:
                citation_count = 0
        rows.append(
            ScholarlyArtifactResolution(
                paper_id=paper_id,
                header_path=str(header_p),
                header_found=header_found,
                citations_path=str(cites_p),
                citations_found=cites_found,
                citation_count=citation_count,
                header_title=header_title,
            )
        )
    return tuple(rows)


def _scholarly_wrapper_summary(
    rows: tuple[ScholarlyArtifactResolution, ...],
) -> dict[str, Any]:
    papers = len(rows)
    headers = sum(1 for r in rows if r.header_found)
    cites_files = sum(1 for r in rows if r.citations_found)
    citation_total = sum(r.citation_count for r in rows)
    return {
        "schema_version": "m218-scholarly-wrapper.v1",
        "papers": papers,
        "headers_found": headers,
        "headers_missing": max(0, papers - headers),
        "citations_files_found": cites_files,
        "citations_files_missing": max(0, papers - cites_files),
        "citation_total": citation_total,
        "complete_wrapper_count": sum(
            1 for r in rows if r.header_found and r.citations_found
        ),
        "per_paper": [r.to_dict() for r in rows],
        "import_eligible": False,
        "graph_writes_allowed": False,
        "source": "grobid_tei_artifacts",
        "note": "candidate-only; not graph import",
    }


def resolve_hybrid_body_paths(
    hybrid_selection: dict[str, Any],
    *,
    body_root: Path,
) -> tuple[BodyPathResolution, ...]:
    """Resolve `{body_root}/{paper_id}/body/{paper_id}.hybrid.body.md` paths.

    Pure path logic (no network). Missing bodies are reported, not invented.
    """
    papers = hybrid_selection.get("papers")
    if not isinstance(papers, list):
        return ()
    rows: list[BodyPathResolution] = []
    for raw in papers:
        if not isinstance(raw, dict):
            continue
        paper_id = str(raw.get("paper_id") or "").strip()
        pdf_path = str(raw.get("pdf_path") or "")
        if not paper_id:
            rows.append(
                BodyPathResolution(
                    paper_id="",
                    pdf_path=pdf_path,
                    body_path=None,
                    found=False,
                )
            )
            continue
        candidate = body_root / paper_id / "body" / f"{paper_id}.hybrid.body.md"
        found = candidate.is_file()
        rows.append(
            BodyPathResolution(
                paper_id=paper_id,
                pdf_path=pdf_path,
                body_path=str(candidate) if found else str(candidate),
                found=found,
            )
        )
    return tuple(rows)


def _combine_verdict(
    *,
    coverage_verdict: str,
    readiness_verdict: str | None,
    missing_body_count: int,
    paper_count: int,
) -> HandoffVerdict:
    if coverage_verdict == "blocked":
        return "blocked"
    if missing_body_count == paper_count and paper_count > 0:
        return "blocked"
    if coverage_verdict == "repair" or missing_body_count > 0:
        return "repair"
    if readiness_verdict == "blocked":
        return "blocked"
    if readiness_verdict == "repair":
        return "repair"
    if readiness_verdict == "ready_for_review" and missing_body_count == 0:
        return "ready_for_review"
    return "repair"


@dataclass(frozen=True, slots=True)
class HybridReadinessHandoffRequest:
    hybrid_selection_path: Path = DEFAULT_SELECTION
    body_root: Path = DEFAULT_BODY_ROOT
    catalog_index_path: Path = DEFAULT_CATALOG_INDEX
    catalog_root: Path = DEFAULT_CATALOG_ROOT
    check_article_json: bool = True
    review_completed: bool = True
    require_min_chunks: int = 1
    output_path: Path | None = None
    repo_root: Path = field(default_factory=lambda: Path("."))
    # When True, skip readiness if no bodies (still emit coverage).
    allow_empty_readiness: bool = True
    # M230: optional YAKE keyword inject at composition boundary (default off).
    use_yake_keywords: bool = False


@dataclass(frozen=True, slots=True)
class HybridReadinessHandoffResult:
    schema_version: str
    handoff_verdict: HandoffVerdict
    coverage: HybridCatalogCoverageResult
    readiness: GraphDataReadinessResult | None
    body_resolutions: tuple[BodyPathResolution, ...]
    bodies_found: int
    bodies_missing: int
    scholarly_resolutions: tuple[ScholarlyArtifactResolution, ...] = ()
    scholarly_wrapper: dict[str, Any] = field(default_factory=dict)
    preprocess_bodies: tuple[dict[str, Any], ...] = ()
    preprocess_rollup: dict[str, Any] = field(default_factory=dict)
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()
    output_path: str | None = None

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("hybrid readiness handoff cannot authorize import or writes")
        if self.coverage.package.report.import_eligible:
            raise ValueError("coverage package cannot authorize import inside handoff")
        if self.readiness is not None and self.readiness.package.import_eligible:
            raise ValueError("readiness package cannot authorize import inside handoff")
        if self.scholarly_wrapper.get("import_eligible") is True:
            raise ValueError("scholarly wrapper cannot authorize import inside handoff")
        for row in self.preprocess_bodies:
            if row.get("import_eligible") is True:
                raise ValueError("preprocess body enrichment cannot authorize import")
        if self.preprocess_rollup.get("import_eligible") is True:
            raise ValueError("preprocess rollup cannot authorize import")
        if self.preprocess_rollup.get("drives_verdict") is True:
            raise ValueError("preprocess rollup cannot drive handoff verdict")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "handoff_verdict": self.handoff_verdict,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "bodies_found": self.bodies_found,
            "bodies_missing": self.bodies_missing,
            "coverage": self.coverage.to_dict(),
            "readiness": {
                "package": self.readiness.package.to_dict() if self.readiness else None,
                "continuity_report_markdown": (
                    self.readiness.continuity_report_markdown if self.readiness else None
                ),
            },
            "body_resolutions": [row.to_dict() for row in self.body_resolutions],
            "scholarly_wrapper": dict(self.scholarly_wrapper),
            "preprocess_bodies": list(self.preprocess_bodies),
            "preprocess_rollup": dict(self.preprocess_rollup),
            "diagnostics": list(self.diagnostics),
            "safety_flags": self.safety_flags.to_dict(),
            "output_path": self.output_path,
        }


def _resolve(path: Path, repo_root: Path) -> Path:
    if path.is_file() or path.is_dir() or path.is_absolute():
        return path
    return repo_root / path


def run_hybrid_readiness_handoff(
    request: HybridReadinessHandoffRequest,
) -> HybridReadinessHandoffResult:
    """Compose catalog coverage + optional readiness over hybrid body artifacts."""
    repo = request.repo_root
    sel_path = _resolve(request.hybrid_selection_path, repo)
    body_root = _resolve(request.body_root, repo)
    if not sel_path.is_file():
        raise FileNotFoundError(f"hybrid selection missing: {sel_path}")

    hybrid_selection = json.loads(sel_path.read_text(encoding="utf-8"))
    bodies = resolve_hybrid_body_paths(hybrid_selection, body_root=body_root)
    found = sum(1 for row in bodies if row.found)
    missing = sum(1 for row in bodies if not row.found)
    scholarly = resolve_scholarly_artifact_paths(
        hybrid_selection, body_root=body_root, load_counts=True
    )
    scholarly_summary = _scholarly_wrapper_summary(scholarly)

    coverage = run_hybrid_catalog_coverage(
        HybridCatalogCoverageRequest(
            hybrid_selection_path=sel_path,
            catalog_index_path=_resolve(request.catalog_index_path, repo),
            catalog_root=_resolve(request.catalog_root, repo),
            check_article_json=request.check_article_json,
            output_path=None,
            repo_root=repo,
        )
    )

    readiness: GraphDataReadinessResult | None = None
    readiness_verdict: str | None = None
    if found > 0:
        sources = tuple(
            SourceInput(
                path=str(row.body_path),
                paper_id=row.paper_id,
                source_type="markdown",
            )
            for row in bodies
            if row.found and row.body_path
        )
        readiness = run_graph_data_readiness_pipeline(
            GraphDataReadinessRequest(
                sources=sources,
                review_completed=request.review_completed,
                repo_root=str(repo),
                require_min_chunks=request.require_min_chunks,
            )
        )
        readiness_verdict = readiness.package.verdict
    elif not request.allow_empty_readiness:
        raise FileNotFoundError(f"no hybrid bodies under {body_root}")

    handoff_verdict = _combine_verdict(
        coverage_verdict=coverage.package.verdict,
        readiness_verdict=readiness_verdict,
        missing_body_count=missing,
        paper_count=len(bodies),
    )

    preprocess_rows: list[dict[str, Any]] = []
    for row in bodies:
        if not row.found or not row.body_path:
            continue
        body_file = Path(row.body_path)
        if not body_file.is_file():
            continue
        try:
            body_text = body_file.read_text(encoding="utf-8")
        except OSError:
            continue
        injected: list[str] | None = None
        yake_lan = ""
        yake_input_chars = 0
        if request.use_yake_keywords:
            # Align YAKE with preprocess cleaned body (markdown, not HTML).
            yake_text = cleaned_body_for_yake(body_text, is_html=False)
            yake_input_chars = len(yake_text)
            detected = detect_text_language(yake_text)
            yake_lan = yake_language_code(detected.language)
            injected = yake_keywords_for_text(
                yake_text, language=yake_lan, top_k=12
            )
        row_summary = preprocess_summary_for_body(
            source_id=row.paper_id,
            text=body_text,
            source_class="arxiv",
            profile="scholarly",
            is_html=False,
            keywords=injected,
        )
        if yake_lan:
            row_summary = {
                **row_summary,
                "yake_language": yake_lan,
                "yake_input_chars": yake_input_chars,
            }
        preprocess_rows.append(row_summary)

    yake_langs = sorted(
        {
            str(r.get("yake_language"))
            for r in preprocess_rows
            if r.get("yake_language")
        }
    )
    preprocess_rollup = rollup_preprocess_bodies(preprocess_rows)
    diagnostics = (
        f"bodies_found:{found}",
        f"bodies_missing:{missing}",
        f"headers_found:{scholarly_summary.get('headers_found')}",
        f"headers_missing:{scholarly_summary.get('headers_missing')}",
        f"citations_files_found:{scholarly_summary.get('citations_files_found')}",
        f"citation_total:{scholarly_summary.get('citation_total')}",
        f"coverage_verdict:{coverage.package.verdict}",
        f"readiness_verdict:{readiness_verdict or 'skipped_no_bodies'}",
        f"handoff_verdict:{handoff_verdict}",
        f"preprocess_bodies:{len(preprocess_rows)}",
        f"preprocess_rollup_bodies:{preprocess_rollup['body_count']}",
        f"preprocess_rollup_drives_verdict:{preprocess_rollup['drives_verdict']}",
        f"use_yake_keywords:{request.use_yake_keywords}",
        f"yake_languages:{','.join(yake_langs) if yake_langs else 'none'}",
        "import_write_fail_closed",
        "no_live_sidecar_start",
        "scholarly_wrapper_candidate_only",
        "preprocess_enrichment_only",
    )

    out_path = request.output_path
    if out_path is not None:
        out_path = _resolve(out_path, repo)
        out_path.parent.mkdir(parents=True, exist_ok=True)

    result = HybridReadinessHandoffResult(
        schema_version=SCHEMA_VERSION,
        handoff_verdict=handoff_verdict,
        coverage=coverage,
        readiness=readiness,
        body_resolutions=bodies,
        bodies_found=found,
        bodies_missing=missing,
        scholarly_resolutions=scholarly,
        scholarly_wrapper=scholarly_summary,
        preprocess_bodies=tuple(preprocess_rows),
        preprocess_rollup=preprocess_rollup,
        diagnostics=diagnostics,
        output_path=str(out_path) if out_path else None,
    )
    if out_path is not None:
        out_path.write_text(
            json.dumps(result.to_dict(), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    return result


__all__ = [
    "DEFAULT_BODY_ROOT",
    "DEFAULT_SELECTION",
    "BodyPathResolution",
    "HybridReadinessHandoffRequest",
    "HybridReadinessHandoffResult",
    "SCHEMA_VERSION",
    "ScholarlyArtifactResolution",
    "resolve_hybrid_body_paths",
    "resolve_scholarly_artifact_paths",
    "run_hybrid_readiness_handoff",
]
