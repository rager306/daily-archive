"""Single-article no-write pipeline (CLI composition helper).

Resolve body via M211 parser body route policy, optionally acquire PDF sidecar,
then run graph-data readiness. Does not authorize graph import/writes.
Never claims hybrid success (ADR-008/009 deferred at runtime).
"""

from __future__ import annotations

import json
import shutil
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from research_graph.application.parser_body_route import BodyPreference, BodyRoute
from research_graph.domain.ports import FullTextProviderPort
from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.infrastructure.corpus.ingestion.fetchers import (
    HTMLDownloader,
    PDFDownloader,
    normalize_arxiv_ref,
)
from research_graph.workflows.composition.graph_data_readiness import (
    GraphDataReadinessRequest,
    GraphDataReadinessResult,
    SourceInput,
    run_graph_data_readiness_pipeline,
)
from research_graph.workflows.composition.parser_body_resolve import (
    ArticleBodyRequest,
    ArticleBodyResult,
    FitzExtractFn,
    resolve_article_body,
)

SourceMode = Literal["auto", "html", "pdf", "local", "mdconverter", "fitz", "hybrid"]


def _mode_to_preference(mode: SourceMode, prefer: Literal["html", "pdf"]) -> BodyPreference:
    if mode in {"html", "mdconverter", "fitz", "hybrid"}:
        return mode  # type: ignore[return-value]
    if mode == "pdf":
        return "mdconverter"
    if mode == "local":
        return "auto"
    # auto
    return "html" if prefer == "html" else "mdconverter"


@dataclass(frozen=True, slots=True)
class SingleArticleRunRequest:
    """One article run: remote arXiv ref/URL or local file path."""

    source: str
    work_dir: Path
    mode: SourceMode = "auto"
    prefer: Literal["html", "pdf"] = "html"
    also_pdf: bool = True
    review_completed: bool = True
    repo_root: Path = field(default_factory=lambda: Path("."))
    allow_network: bool = True


@dataclass(frozen=True, slots=True)
class SingleArticleRunResult:
    paper_id: str
    local_sources: tuple[dict[str, str], ...]
    readiness: GraphDataReadinessResult | None
    package_path: Path | None
    continuity_report_path: Path | None
    body_route: BodyRoute
    body: ArticleBodyResult
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        self.body.safety_flags.assert_no_write()
        if self.readiness is not None:
            self.readiness.package.safety_flags.assert_no_write()
        if self.body.decision.hybrid_claimed_success:
            raise ValueError("single article result cannot claim hybrid success")

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "body_route": self.body_route,
            "body": self.body.to_dict(),
            "local_sources": list(self.local_sources),
            "package": self.readiness.package.to_dict() if self.readiness else None,
            "package_path": str(self.package_path) if self.package_path else None,
            "continuity_report_path": (
                str(self.continuity_report_path) if self.continuity_report_path else None
            ),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "production_import_attempted": False,
            "falkor_touched": False,
            "hybrid_claimed_success": False,
            "safety_flags": self.safety_flags.to_dict(),
        }


def _copy_into_work(src: Path, dest: Path) -> Path:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if src.resolve() != dest.resolve():
        shutil.copy2(src, dest)
    return dest


def _maybe_acquire_pdf_sidecar(
    request: SingleArticleRunRequest,
    *,
    paper_id: str,
    pdf_downloader: PDFDownloader | None,
) -> list[dict[str, str]]:
    if not request.also_pdf:
        return []
    source_path = Path(request.source)
    if source_path.is_file() and source_path.suffix.lower() == ".pdf":
        dest = request.work_dir / "source" / source_path.name
        _copy_into_work(source_path.resolve(), dest)
        return [{"kind": "pdf", "path": str(dest), "origin": "local"}]
    if not request.allow_network:
        return []
    try:
        arxiv_id = normalize_arxiv_ref(request.source)
    except ValueError:
        return []
    pdf_dl = pdf_downloader or PDFDownloader(cache_dir=request.work_dir / "cache" / "pdf")
    pdf_cached = pdf_dl.download(arxiv_id)
    pdf_dest = request.work_dir / "source" / f"{arxiv_id}.pdf"
    _copy_into_work(Path(pdf_cached), pdf_dest)
    return [{"kind": "pdf", "path": str(pdf_dest), "origin": "arxiv_pdf"}]


def run_single_article_pipeline(
    request: SingleArticleRunRequest,
    *,
    html_downloader: HTMLDownloader | None = None,
    pdf_downloader: PDFDownloader | None = None,
    fulltext_provider: FullTextProviderPort | None = None,
    fitz_extract: FitzExtractFn | None = None,
    write_artifacts: bool = True,
) -> SingleArticleRunResult:
    """Resolve body route, acquire optional PDF sidecar, run no-write readiness."""
    preference = _mode_to_preference(request.mode, request.prefer)
    body = resolve_article_body(
        ArticleBodyRequest(
            source=request.source,
            work_dir=request.work_dir,
            preference=preference,
            allow_network=request.allow_network,
            fitz_fallback_allowed=True,
        ),
        fulltext_provider=fulltext_provider,
        html_downloader=html_downloader,
        fitz_extract=fitz_extract,
    )
    paper_id = body.paper_id
    records: list[dict[str, str]] = []
    if body.body_path is not None:
        records.append(
            {
                "kind": body.body_source_type,
                "path": str(body.body_path),
                "origin": f"body_route:{body.route}",
            }
        )
    records.extend(
        _maybe_acquire_pdf_sidecar(
            request, paper_id=paper_id, pdf_downloader=pdf_downloader
        )
    )

    readiness: GraphDataReadinessResult | None = None
    if body.body_path is not None and body.body_chars > 0:
        source_type = (
            "html"
            if body.body_source_type == "html"
            else ("markdown" if body.body_source_type == "markdown" else "text")
        )
        readiness = run_graph_data_readiness_pipeline(
            GraphDataReadinessRequest(
                sources=(
                    SourceInput(
                        path=str(body.body_path),
                        paper_id=paper_id,
                        source_type=source_type,
                    ),
                ),
                review_completed=request.review_completed,
                repo_root=str(request.repo_root),
                require_min_chunks=1,
            )
        )

    package_path: Path | None = None
    continuity_path: Path | None = None
    if write_artifacts:
        out = request.work_dir / "readiness"
        out.mkdir(parents=True, exist_ok=True)
        package_path = out / "package.json"
        continuity_path = out / "continuity.md"
        payload = {
            "paper_id": paper_id,
            "body_route": body.route,
            "body": body.to_dict(),
            "package": readiness.package.to_dict() if readiness else None,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "hybrid_claimed_success": False,
            "sources": records,
        }
        package_path.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        if readiness is not None:
            continuity_path.write_text(
                readiness.continuity_report_markdown, encoding="utf-8"
            )
        else:
            continuity_path.write_text(
                f"# Continuity\n\nbody_route: `{body.route}`\n\nno readiness package "
                f"(body unresolved or hybrid_deferred)\n",
                encoding="utf-8",
            )
        (out / "sources.json").write_text(
            json.dumps(
                {
                    "paper_id": paper_id,
                    "body_route": body.route,
                    "sources": records,
                    "import_eligible": False,
                    "graph_writes_allowed": False,
                    "hybrid_claimed_success": False,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )

    if readiness is None and body.route not in {"hybrid_deferred", "unavailable"}:
        raise ValueError(
            f"body route {body.route} produced no readiness body "
            f"(paper_id={paper_id}; diagnostics={body.diagnostics})"
        )

    return SingleArticleRunResult(
        paper_id=paper_id,
        local_sources=tuple(records),
        readiness=readiness,
        package_path=package_path,
        continuity_report_path=continuity_path,
        body_route=body.route,
        body=body,
    )


__all__ = [
    "SingleArticleRunRequest",
    "SingleArticleRunResult",
    "SourceMode",
    "run_single_article_pipeline",
]
