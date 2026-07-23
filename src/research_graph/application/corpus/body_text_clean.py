"""Pure deterministic body text clean helpers (M224 S01).

Patterns inspired by quant-mind ``preprocess/clean.py`` (unicode, whitespace,
consecutive line dedupe). Reimplemented here — no vendor import.

Application-layer pure functions: no I/O, no LLM, no graph writes.
Never authorizes import.
"""

from __future__ import annotations

import re
import unicodedata
from dataclasses import dataclass

# Conservative ligature / smart-punctuation map (searchable, model-friendly).
_LIGATURE_MAP: dict[str, str] = {
    "\ufb00": "ff",
    "\ufb01": "fi",
    "\ufb02": "fl",
    "\ufb03": "ffi",
    "\ufb04": "ffl",
    "\u2018": "'",
    "\u2019": "'",
    "\u201c": '"',
    "\u201d": '"',
    "\u2013": "-",
    "\u2014": "-",
    "\u2026": "...",
    "\u00a0": " ",
}

_LIGATURE_RE = re.compile("|".join(re.escape(k) for k in _LIGATURE_MAP))
_HORIZONTAL_WS_RE = re.compile(r"[ \t\f\v]+")
_TRIPLE_NEWLINE_RE = re.compile(r"\n{3,}")
_CONTROL_RE = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")


@dataclass(frozen=True, slots=True)
class BodyCleanResult:
    """Cleaned body text plus applied ops. Always import-blocked."""

    text: str
    ops: tuple[str, ...] = ()
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("body clean cannot authorize import/writes")


def normalize_unicode(text: str) -> str:
    """NFKC + ligature/smart-quote map + drop control chars (keep \\n/\\t)."""
    if not text:
        return ""
    normalized = unicodedata.normalize("NFKC", text)
    normalized = _LIGATURE_RE.sub(lambda m: _LIGATURE_MAP[m.group(0)], normalized)
    return _CONTROL_RE.sub("", normalized)


def collapse_whitespace(text: str) -> str:
    """Collapse horizontal whitespace; keep paragraph breaks (max double newline)."""
    if not text:
        return ""
    collapsed = _HORIZONTAL_WS_RE.sub(" ", text)
    collapsed = _TRIPLE_NEWLINE_RE.sub("\n\n", collapsed)
    lines = [line.rstrip() for line in collapsed.split("\n")]
    return "\n".join(lines).strip()


def dedupe_consecutive_lines(text: str) -> str:
    """Drop consecutive duplicate lines (PDF headers/footers). Non-consecutive kept."""
    if not text:
        return ""
    output: list[str] = []
    last: str | None = None
    for line in text.split("\n"):
        key = line.strip()
        if key and key == last:
            continue
        output.append(line)
        last = key if key else None
    return "\n".join(output)


def clean_body_text(text: str) -> BodyCleanResult:
    """Compose normalize → collapse → dedupe. Fail-closed import flags."""
    if not text:
        return BodyCleanResult(text="", ops=())
    ops: list[str] = []
    out = normalize_unicode(text)
    ops.append("normalize_unicode")
    out = collapse_whitespace(out)
    ops.append("collapse_whitespace")
    out = dedupe_consecutive_lines(out)
    ops.append("dedupe_consecutive_lines")
    return BodyCleanResult(text=out, ops=tuple(ops))


__all__ = [
    "BodyCleanResult",
    "clean_body_text",
    "collapse_whitespace",
    "dedupe_consecutive_lines",
    "normalize_unicode",
]
