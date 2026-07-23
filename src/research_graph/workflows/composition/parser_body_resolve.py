"""Resolve article body for graph-prep (M211/M212 composition).

Executes BodyRouteDecision via injected ports/callables.
Hybrid ADR-008/009 success only when hybrid runtime returns body evidence;
otherwise hybrid_deferred. May use network only through injected downloaders /
FullTextProviderPort / hybrid sidecar ports.
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Protocol

from research_graph.application.parser_body_route import (
    BODY_RESOLVE_STAGE_NAME,
    BodyPreference,
    BodyRoute,
    BodyRouteDecision,
    BodyRouteIntent,
    decide_body_route,
)
from research_graph.domain.ports import ConversionResult, FullTextProviderPort
from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.infrastructure.corpus.ingestion.fetchers import (
    HTMLDownloader,
    normalize_arxiv_ref,
)
from research_graph.infrastructure.corpus.ingestion.loader import load_article_source
from research_graph.workflows.composition.hybrid_sidecar_runtime import (
    GrobidSidecarPort,
    HybridRuntimeRequest,
    OpenDataLoaderSidecarPort,
    run_hybrid_sidecar_runtime,
)

FitzExtractFn = Callable[[Path, Path], Path]


class _HtmlDownloaderLike(Protocol):
    def download(self, arxiv_id: str, html_url: str | None = None) -> Path: ...


@dataclass(frozen=True, slots=True)
class ArticleBodyRequest:
    source: str
    work_dir: Path
    preference: BodyPreference = "auto"
    paper_id: str | None = None
    allow_network: bool = True
    fitz_fallback_allowed: bool = True


@dataclass(frozen=True, slots=True)
class ArticleBodyResult:
    paper_id: str
    route: BodyRoute
    decision: BodyRouteDecision
    body_path: Path | None
    body_source_type: str
    body_chars: int
    diagnostics: tuple[str, ...] = ()
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        self.decision.safety_flags.assert_no_write()
        # Policy decision never claims hybrid; result may after packet evidence.
        if self.decision.hybrid_claimed_success:
            raise ValueError("body resolve decision cannot claim hybrid success")

    def to_dict(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "route": self.route,
            "decision": self.decision.to_dict(),
            "body_path": str(self.body_path) if self.body_path else None,
            "body_source_type": self.body_source_type,
            "body_chars": self.body_chars,
            "diagnostics": list(self.diagnostics),
            "stage_name": BODY_RESOLVE_STAGE_NAME,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "safety_flags": self.safety_flags.to_dict(),
        }


def _persist_grobid_structured_artifacts(
    *,
    body_dir: Path,
    paper_id: str,
    grobid_metrics: dict[str, Any] | None,
) -> list[str]:
    """Write hybrid.header.json + hybrid.citations.jsonl when GROBID structured payload exists.

    Candidate-only artifacts; never sets import/write true.
    """
    import json

    diag: list[str] = []
    if not isinstance(grobid_metrics, dict):
        return ["grobid_structured_absent"]
    header = grobid_metrics.get("header")
    citations = grobid_metrics.get("citations")
    if not isinstance(header, dict) and not isinstance(citations, list):
        return ["grobid_structured_absent"]

    body_dir.mkdir(parents=True, exist_ok=True)
    if isinstance(header, dict):
        # Force fail-closed flags on disk
        payload = dict(header)
        payload["import_eligible"] = False
        payload["graph_writes_allowed"] = False
        header_path = body_dir / f"{paper_id}.hybrid.header.json"
        header_path.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        diag.append(f"grobid_header_artifact:{header_path.name}")
    if isinstance(citations, list):
        cites_path = body_dir / f"{paper_id}.hybrid.citations.jsonl"
        lines: list[str] = []
        for row in citations:
            if not isinstance(row, dict):
                continue
            item = dict(row)
            item["import_eligible"] = False
            item["graph_writes_allowed"] = False
            lines.append(json.dumps(item, sort_keys=True))
        cites_path.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")
        diag.append(f"grobid_citations_artifact:{cites_path.name}")
        diag.append(f"grobid_citation_rows:{len(lines)}")
    return diag


def _is_local_file(source: str) -> bool:
    path = Path(source)
    return path.is_file() or source.startswith(("./", "../", "/"))


def _probe_intent(
    request: ArticleBodyRequest,
    *,
    fulltext_provider: FullTextProviderPort | None,
    hybrid_runtime_available: bool = False,
) -> tuple[str, BodyRouteIntent, Path | None]:
    """Build routing intent and optional local path / arxiv id."""
    local_path: Path | None = None
    arxiv_id: str | None = None
    has_html = has_md = has_pdf = False
    paper_id: str

    if _is_local_file(request.source):
        local_path = Path(request.source).expanduser().resolve()
        if not local_path.is_file():
            raise FileNotFoundError(f"local source not found: {local_path}")
        suffix = local_path.suffix.lower()
        has_html = suffix in {".html", ".htm"}
        has_md = suffix in {".md", ".markdown", ".txt"}
        has_pdf = suffix == ".pdf"
        paper_id = request.paper_id or local_path.stem
    else:
        try:
            arxiv_id = normalize_arxiv_ref(request.source)
        except ValueError as exc:
            raise ValueError(f"unrecognized article source: {request.source!r}") from exc
        paper_id = request.paper_id or arxiv_id

    intent = BodyRouteIntent(
        preference=request.preference,
        has_local_html=has_html,
        has_local_markdown=has_md,
        has_local_pdf=has_pdf,
        has_arxiv_id=arxiv_id is not None,
        fulltext_provider_available=fulltext_provider is not None,
        fitz_fallback_allowed=request.fitz_fallback_allowed,
        hybrid_runtime_available=hybrid_runtime_available,
    )
    return paper_id, intent, local_path


def resolve_article_body(
    request: ArticleBodyRequest,
    *,
    fulltext_provider: FullTextProviderPort | None = None,
    html_downloader: _HtmlDownloaderLike | None = None,
    fitz_extract: FitzExtractFn | None = None,
    grobid: GrobidSidecarPort | None = None,
    opendataloader: OpenDataLoaderSidecarPort | None = None,
    hybrid_pdf_path: Path | None = None,
) -> ArticleBodyResult:
    """Resolve a body text file path + honest route diagnostics."""
    work = request.work_dir
    body_dir = work / "body"
    body_dir.mkdir(parents=True, exist_ok=True)
    hybrid_available = grobid is not None or opendataloader is not None
    paper_id, intent, local_path = _probe_intent(
        request,
        fulltext_provider=fulltext_provider,
        hybrid_runtime_available=hybrid_available,
    )
    decision = decide_body_route(intent)
    diagnostics: list[str] = list(decision.diagnostics)

    if decision.route == "hybrid":
        pdf_path = hybrid_pdf_path
        if pdf_path is None and local_path is not None and local_path.suffix.lower() == ".pdf":
            pdf_path = local_path
        runtime = run_hybrid_sidecar_runtime(
            HybridRuntimeRequest(paper_id=paper_id, pdf_path=pdf_path),
            grobid=grobid,
            opendataloader=opendataloader,
        )
        diagnostics.extend(runtime.diagnostics)
        packet = runtime.packet
        # Persist GROBID structured candidates when present (M217), even if body later fails.
        grobid_artifact_diag = _persist_grobid_structured_artifacts(
            body_dir=body_dir,
            paper_id=paper_id,
            grobid_metrics=runtime.grobid_metrics,
        )
        diagnostics.extend(grobid_artifact_diag)
        if packet.hybrid_claimed_success and packet.body_markdown:
            text_path = body_dir / f"{paper_id}.hybrid.body.md"
            text_path.write_text(packet.body_markdown, encoding="utf-8")
            return ArticleBodyResult(
                paper_id=paper_id,
                route="hybrid",
                decision=decision,
                body_path=text_path,
                body_source_type="markdown",
                body_chars=packet.body_chars,
                diagnostics=tuple(diagnostics)
                + (f"hybrid_route:{packet.route}", "hybrid_body_from_packet"),
            )
        # Honest fallback: no body evidence
        return ArticleBodyResult(
            paper_id=paper_id,
            route="hybrid_deferred",
            decision=decision,
            body_path=None,
            body_source_type="none",
            body_chars=0,
            diagnostics=tuple(diagnostics)
            + (
                f"hybrid_route:{packet.route}",
                "hybrid_no_body_evidence",
                "do_not_claim_hybrid_success",
            ),
        )

    if decision.route == "hybrid_deferred":
        return ArticleBodyResult(
            paper_id=paper_id,
            route=decision.route,
            decision=decision,
            body_path=None,
            body_source_type="none",
            body_chars=0,
            diagnostics=tuple(diagnostics)
            + ("hybrid_deferred_no_body", "use_html_or_mdconverter_instead"),
        )

    if decision.route == "unavailable":
        return ArticleBodyResult(
            paper_id=paper_id,
            route=decision.route,
            decision=decision,
            body_path=None,
            body_source_type="none",
            body_chars=0,
            diagnostics=tuple(diagnostics) + ("body_unavailable",),
        )

    if decision.route == "html_native":
        if local_path is not None and local_path.suffix.lower() in {
            ".html",
            ".htm",
            ".md",
            ".markdown",
            ".txt",
        }:
            dest = body_dir / local_path.name
            if local_path.resolve() != dest.resolve():
                dest.write_bytes(local_path.read_bytes())
            load = load_article_source(
                dest,
                source_type="html"
                if dest.suffix.lower() in {".html", ".htm"}
                else ("markdown" if dest.suffix.lower() in {".md", ".markdown"} else "text"),
                paper_id=paper_id,
            )
            if load.outcome != "loaded" or not load.text:
                return ArticleBodyResult(
                    paper_id=paper_id,
                    route="unavailable",
                    decision=decision,
                    body_path=None,
                    body_source_type=str(load.source_type),
                    body_chars=0,
                    diagnostics=tuple(diagnostics)
                    + (f"load_failed:{load.outcome}", str(load.failure_reason or "")),
                )
            text_path = body_dir / f"{paper_id}.body.txt"
            text_path.write_text(load.text, encoding="utf-8")
            return ArticleBodyResult(
                paper_id=paper_id,
                route="html_native",
                decision=decision,
                body_path=text_path,
                body_source_type=str(load.source_type),
                body_chars=len(load.text),
                diagnostics=tuple(diagnostics) + ("local_body_loaded",),
            )

        # remote arxiv html
        if not request.allow_network:
            return ArticleBodyResult(
                paper_id=paper_id,
                route="unavailable",
                decision=decision,
                body_path=None,
                body_source_type="html",
                body_chars=0,
                diagnostics=tuple(diagnostics) + ("network_disabled",),
            )
        arxiv_id = normalize_arxiv_ref(request.source)
        dl = html_downloader or HTMLDownloader(cache_dir=work / "cache" / "html")
        html_path = dl.download(arxiv_id)
        dest = body_dir / "article.html"
        dest.write_bytes(Path(html_path).read_bytes())
        load = load_article_source(dest, source_type="html", paper_id=paper_id)
        if load.outcome != "loaded" or not load.text:
            return ArticleBodyResult(
                paper_id=paper_id,
                route="unavailable",
                decision=decision,
                body_path=None,
                body_source_type="html",
                body_chars=0,
                diagnostics=tuple(diagnostics)
                + (f"html_load_failed:{load.outcome}", str(load.failure_reason or "")),
            )
        text_path = body_dir / f"{paper_id}.body.txt"
        text_path.write_text(load.text, encoding="utf-8")
        return ArticleBodyResult(
            paper_id=paper_id,
            route="html_native",
            decision=decision,
            body_path=text_path,
            body_source_type="html",
            body_chars=len(load.text),
            diagnostics=tuple(diagnostics) + ("arxiv_html_loaded",),
        )

    if decision.route == "mdconverter":
        if fulltext_provider is None:
            return ArticleBodyResult(
                paper_id=paper_id,
                route="unavailable",
                decision=decision,
                body_path=None,
                body_source_type="markdown",
                body_chars=0,
                diagnostics=tuple(diagnostics) + ("fulltext_provider_missing",),
            )
        arxiv_id = paper_id if local_path is None else normalize_arxiv_ref(paper_id)
        if local_path is None:
            arxiv_id = normalize_arxiv_ref(request.source)
        result: ConversionResult = fulltext_provider.convert_sync(arxiv_id)
        if result.error or not result.markdown:
            return ArticleBodyResult(
                paper_id=paper_id,
                route="unavailable",
                decision=decision,
                body_path=None,
                body_source_type="markdown",
                body_chars=0,
                diagnostics=tuple(diagnostics)
                + (
                    f"mdconverter_error:{result.error or 'empty'}",
                    f"method:{result.method}",
                ),
            )
        text_path = body_dir / f"{paper_id}.mdconverter.md"
        text_path.write_text(result.markdown, encoding="utf-8")
        return ArticleBodyResult(
            paper_id=paper_id,
            route="mdconverter",
            decision=decision,
            body_path=text_path,
            body_source_type="markdown",
            body_chars=len(result.markdown),
            diagnostics=tuple(diagnostics)
            + (f"mdconverter_method:{result.method}", "not_hybrid"),
        )

    if decision.route == "fitz_offline":
        if local_path is None or local_path.suffix.lower() != ".pdf":
            return ArticleBodyResult(
                paper_id=paper_id,
                route="unavailable",
                decision=decision,
                body_path=None,
                body_source_type="pdf",
                body_chars=0,
                diagnostics=tuple(diagnostics) + ("fitz_needs_local_pdf",),
            )
        if fitz_extract is None:
            return ArticleBodyResult(
                paper_id=paper_id,
                route="unavailable",
                decision=decision,
                body_path=None,
                body_source_type="pdf",
                body_chars=0,
                diagnostics=tuple(diagnostics)
                + ("fitz_extract_not_injected", "not_default_hybrid_replacement"),
            )
        cache_path = body_dir / f"{paper_id}.fitz.txt"
        out = fitz_extract(local_path, cache_path)
        text = Path(out).read_text(encoding="utf-8", errors="replace")
        return ArticleBodyResult(
            paper_id=paper_id,
            route="fitz_offline",
            decision=decision,
            body_path=Path(out),
            body_source_type="text",
            body_chars=len(text),
            diagnostics=tuple(diagnostics) + ("fitz_offline_body", "not_hybrid"),
        )

    return ArticleBodyResult(
        paper_id=paper_id,
        route="unavailable",
        decision=decision,
        body_path=None,
        body_source_type="none",
        body_chars=0,
        diagnostics=tuple(diagnostics) + ("unhandled_route",),
    )


__all__ = [
    "ArticleBodyRequest",
    "ArticleBodyResult",
    "FitzExtractFn",
    "resolve_article_body",
]
