"""Resolve article body for graph-prep (M211 composition).

Executes BodyRouteDecision via injected ports/callables. Hybrid ADR-008/009 remains
deferred: this module never reports hybrid success. May use network only through
injected downloaders / FullTextProviderPort.
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
        if self.decision.hybrid_claimed_success:
            raise ValueError("body resolve cannot claim hybrid success")

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


def _is_local_file(source: str) -> bool:
    path = Path(source)
    return path.is_file() or source.startswith(("./", "../", "/"))


def _probe_intent(
    request: ArticleBodyRequest,
    *,
    fulltext_provider: FullTextProviderPort | None,
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
        hybrid_runtime_available=False,
    )
    return paper_id, intent, local_path


def resolve_article_body(
    request: ArticleBodyRequest,
    *,
    fulltext_provider: FullTextProviderPort | None = None,
    html_downloader: _HtmlDownloaderLike | None = None,
    fitz_extract: FitzExtractFn | None = None,
) -> ArticleBodyResult:
    """Resolve a body text file path + honest route diagnostics."""
    work = request.work_dir
    body_dir = work / "body"
    body_dir.mkdir(parents=True, exist_ok=True)
    paper_id, intent, local_path = _probe_intent(request, fulltext_provider=fulltext_provider)
    decision = decide_body_route(intent)
    diagnostics: list[str] = list(decision.diagnostics)

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
