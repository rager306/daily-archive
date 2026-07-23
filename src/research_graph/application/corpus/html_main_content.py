"""Stdlib HTML main-content extraction (M224 S03).

Prefers ``<article>`` / ``<main>`` regions; skips nav/footer/aside/script/style.
Application pure — no BeautifulSoup, no network. Never authorizes import.

# ponytail: heuristic region pick (not full readability), upgrade path: optional
# trafilatura-style scoring if HTML corpus quality demands it.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from html.parser import HTMLParser

_SKIP_CHROME = frozenset({"nav", "footer", "aside", "script", "style", "noscript", "template"})
_PREFERRED = frozenset({"article", "main"})
_BLOCK = frozenset(
    {
        "p",
        "div",
        "section",
        "li",
        "ul",
        "ol",
        "br",
        "tr",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
    }
)
_HEADING = {"h1": "#", "h2": "##", "h3": "###", "h4": "####", "h5": "#####", "h6": "######"}
_WS_RE = re.compile(r"[ \t]+")
_NL_RE = re.compile(r"\n{3,}")


@dataclass(frozen=True, slots=True)
class HtmlMainContentResult:
    """Main-content extraction outcome. Always import-blocked."""

    text: str
    region: str
    main_content_ratio: float | None
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("html main content cannot authorize import/writes")


class _RegionCollector(HTMLParser):
    """Collect text per region: preferred (article/main), body, chrome-skipped."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self._skip_depth = 0
        self._preferred_depth = 0
        self._in_body = 0
        self._pending_heading: str | None = None
        self.preferred_parts: list[str] = []
        self.body_parts: list[str] = []
        self.all_parts: list[str] = []
        self.saw_preferred = False

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        tag_l = tag.lower()
        if tag_l in _SKIP_CHROME:
            self._skip_depth += 1
            return
        if self._skip_depth:
            return
        if tag_l in _PREFERRED:
            self._preferred_depth += 1
            self.saw_preferred = True
        if tag_l == "body":
            self._in_body += 1
        if tag_l in _HEADING:
            self._pending_heading = _HEADING[tag_l]
            self._emit("\n")
        elif tag_l in _BLOCK:
            self._emit("\n")

    def handle_endtag(self, tag: str) -> None:
        tag_l = tag.lower()
        if tag_l in _SKIP_CHROME and self._skip_depth:
            self._skip_depth -= 1
            return
        if self._skip_depth:
            return
        if tag_l in _PREFERRED and self._preferred_depth:
            self._preferred_depth -= 1
        if tag_l == "body" and self._in_body:
            self._in_body -= 1
        if tag_l in _HEADING:
            self._pending_heading = None
            self._emit("\n")
        elif tag_l in _BLOCK:
            self._emit("\n")

    def handle_data(self, data: str) -> None:
        if self._skip_depth:
            return
        text = data.strip()
        if not text:
            return
        if self._pending_heading:
            chunk = f"{self._pending_heading} {text}"
            self._pending_heading = None
        else:
            chunk = text + " "
        self._emit(chunk)

    def _emit(self, chunk: str) -> None:
        self.all_parts.append(chunk)
        if self._preferred_depth > 0:
            self.preferred_parts.append(chunk)
        if self._in_body > 0 or self._preferred_depth > 0:
            self.body_parts.append(chunk)


def _join(parts: list[str]) -> str:
    joined = "".join(parts)
    joined = _WS_RE.sub(" ", joined)
    joined = _NL_RE.sub("\n\n", joined)
    return joined.strip()


def extract_html_main_content(raw_html: str) -> HtmlMainContentResult:
    """Extract main textual content from HTML; drop nav/footer/aside chrome."""
    if not raw_html.strip():
        return HtmlMainContentResult(text="", region="empty", main_content_ratio=None)

    parser = _RegionCollector()
    try:
        parser.feed(raw_html)
        parser.close()
    except Exception:  # noqa: BLE001 - fail-closed parse
        return HtmlMainContentResult(text="", region="parse_error", main_content_ratio=None)

    preferred = _join(parser.preferred_parts)
    body = _join(parser.body_parts)
    full = _join(parser.all_parts)

    if preferred:
        text, region = preferred, "article_or_main"
    elif body:
        text, region = body, "body"
    else:
        text, region = full, "full"

    ratio: float | None = None
    if full and text:
        ratio = round(min(1.0, len(text) / max(len(full), 1)), 4)
    elif not text:
        ratio = 0.0

    return HtmlMainContentResult(text=text, region=region, main_content_ratio=ratio)


__all__ = ["HtmlMainContentResult", "extract_html_main_content"]
