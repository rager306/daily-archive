"""Single-article no-write pipeline (CLI composition helper).

Acquire one arXiv HTML/PDF (or use a local path), then run the existing
graph-data readiness composition root. Does not authorize graph import/writes.
"""

from __future__ import annotations

import json
import shutil
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

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

SourceMode = Literal["auto", "html", "pdf", "local"]


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


@dataclass(frozen=True, slots=True)
class SingleArticleRunResult:
    paper_id: str
    local_sources: tuple[dict[str, str], ...]
    readiness: GraphDataReadinessResult
    package_path: Path | None
    continuity_report_path: Path | None
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        self.readiness.package.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "local_sources": list(self.local_sources),
            "package": self.readiness.package.to_dict(),
            "package_path": str(self.package_path) if self.package_path else None,
            "continuity_report_path": (
                str(self.continuity_report_path) if self.continuity_report_path else None
            ),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "production_import_attempted": False,
            "falkor_touched": False,
            "safety_flags": self.safety_flags.to_dict(),
        }


def _looks_like_local_path(source: str) -> bool:
    path = Path(source)
    return path.exists() or source.startswith(("./", "../", "/")) or path.suffix.lower() in {
        ".html",
        ".htm",
        ".md",
        ".markdown",
        ".txt",
        ".pdf",
    }


def _copy_into_work(src: Path, dest: Path) -> Path:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if src.resolve() != dest.resolve():
        shutil.copy2(src, dest)
    return dest


def resolve_local_sources(
    request: SingleArticleRunRequest,
    *,
    html_downloader: HTMLDownloader | None = None,
    pdf_downloader: PDFDownloader | None = None,
) -> tuple[str, list[SourceInput], list[dict[str, str]]]:
    """Resolve source to local files under work_dir; may use network for arXiv."""
    work = request.work_dir
    source_dir = work / "source"
    source_dir.mkdir(parents=True, exist_ok=True)
    records: list[dict[str, str]] = []
    inputs: list[SourceInput] = []

    if request.mode == "local" or _looks_like_local_path(request.source):
        path = Path(request.source).expanduser().resolve()
        if not path.is_file():
            raise FileNotFoundError(f"local source not found: {path}")
        paper_id = path.stem
        dest = source_dir / path.name
        _copy_into_work(path, dest)
        st = "html" if path.suffix.lower() in {".html", ".htm"} else "auto"
        if path.suffix.lower() == ".pdf":
            # PDF body is metadata-only in core loader; still record the file.
            records.append({"kind": "pdf", "path": str(dest), "origin": "local"})
            return paper_id, inputs, records
        records.append({"kind": st if st != "auto" else "text", "path": str(dest), "origin": "local"})
        inputs.append(SourceInput(path=str(dest), paper_id=paper_id, source_type=st))
        return paper_id, inputs, records

    paper_id = normalize_arxiv_ref(request.source)
    prefer = request.prefer if request.mode == "auto" else request.mode  # type: ignore[assignment]
    if prefer not in {"html", "pdf"}:
        prefer = "html"

    if prefer == "html" or request.mode == "html":
        html_dl = html_downloader or HTMLDownloader(cache_dir=work / "cache" / "html")
        html_cached = html_dl.download(paper_id)
        html_dest = source_dir / "article.html"
        _copy_into_work(html_cached, html_dest)
        records.append({"kind": "html", "path": str(html_dest), "origin": "arxiv_html"})
        inputs.append(SourceInput(path=str(html_dest), paper_id=paper_id, source_type="html"))

    if request.mode == "pdf" or request.also_pdf or prefer == "pdf":
        pdf_dl = pdf_downloader or PDFDownloader(cache_dir=work / "cache" / "pdf")
        pdf_cached = pdf_dl.download(paper_id)
        pdf_dest = source_dir / f"{paper_id}.pdf"
        _copy_into_work(pdf_cached, pdf_dest)
        records.append({"kind": "pdf", "path": str(pdf_dest), "origin": "arxiv_pdf"})
        # PDF is not fed into readiness body path (metadata-only core loader).

    if not inputs and prefer == "pdf":
        # No HTML body; readiness cannot structure PDF metadata-only as fulltext.
        # Caller still gets acquired PDF path in records.
        pass

    return paper_id, inputs, records


def run_single_article_pipeline(
    request: SingleArticleRunRequest,
    *,
    html_downloader: HTMLDownloader | None = None,
    pdf_downloader: PDFDownloader | None = None,
    write_artifacts: bool = True,
) -> SingleArticleRunResult:
    """Acquire (if needed) and run no-write readiness for one article."""
    paper_id, inputs, records = resolve_local_sources(
        request,
        html_downloader=html_downloader,
        pdf_downloader=pdf_downloader,
    )
    if not inputs:
        raise ValueError(
            "no full-text source available for readiness "
            f"(paper_id={paper_id}; acquired={[r['kind'] for r in records]})"
        )

    readiness = run_graph_data_readiness_pipeline(
        GraphDataReadinessRequest(
            sources=tuple(inputs),
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
        package_path.write_text(
            json.dumps(readiness.package.to_dict(), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        continuity_path.write_text(readiness.continuity_report_markdown, encoding="utf-8")
        (out / "sources.json").write_text(
            json.dumps(
                {
                    "paper_id": paper_id,
                    "sources": records,
                    "import_eligible": False,
                    "graph_writes_allowed": False,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )

    return SingleArticleRunResult(
        paper_id=paper_id,
        local_sources=tuple(records),
        readiness=readiness,
        package_path=package_path,
        continuity_report_path=continuity_path,
    )


__all__ = [
    "SingleArticleRunRequest",
    "SingleArticleRunResult",
    "SourceMode",
    "resolve_local_sources",
    "run_single_article_pipeline",
]
