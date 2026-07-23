"""Profile-scoped body quality diagnostics (M224 S02).

Soft diagnostics only — never authorizes import. Inspired by yago
Gopher/C4-style content quality (word bounds, symbol ratio, n-gram
repetition) but profile-scoped:

- ``web``: stricter floors (junk/chrome HTML risk)
- ``scholarly``: soft scores; short abstracts are not hard-dropped

Distinct from :func:`assess_full_text_quality` (landing-page / heading-only
check in the loader). Application pure: no I/O, no LLM.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Literal

BodyQualityProfile = Literal["web", "scholarly"]

_WORD_RE = re.compile(r"[A-Za-z0-9\u00C0-\u024F]+(?:['’-][A-Za-z0-9\u00C0-\u024F]+)*")
_SYMBOL_RE = re.compile(r"[#…\.\!\?\$%\*@]{1,}")

# Web profile thresholds (yago/Gopher-inspired, simplified).
_WEB_MIN_WORDS = 50
_SCHOLARLY_MIN_WORDS = 15  # soft only: below this → soft_signal, not hard drop
_MAX_MEAN_WORD_LEN = 12.0
_MIN_MEAN_WORD_LEN = 2.5
_MAX_SYMBOL_RATIO = 0.12
_NGRAM_REPEAT_SHARE = 0.22


@dataclass(frozen=True, slots=True)
class BodyQualityReport:
    """Named quality diagnostics for one body text. Always import-blocked."""

    profile: BodyQualityProfile
    status: str
    word_count: int
    scores: dict[str, float]
    rule_hits: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("body quality cannot authorize import/writes")


def _tokens(text: str) -> list[str]:
    return [m.group(0).casefold() for m in _WORD_RE.finditer(text)]


def _mean_word_len(tokens: list[str]) -> float:
    if not tokens:
        return 0.0
    return sum(len(t) for t in tokens) / len(tokens)


def _symbol_ratio(text: str, word_count: int) -> float:
    if word_count <= 0:
        return 1.0 if text.strip() else 0.0
    symbols = len(_SYMBOL_RE.findall(text))
    return symbols / max(word_count, 1)


def _ngram_repetition_share(tokens: list[str], n: int = 3) -> float:
    """Share of character mass covered by the most frequent n-gram (approx)."""
    if len(tokens) < n * 2:
        return 0.0
    grams: dict[tuple[str, ...], int] = {}
    for i in range(len(tokens) - n + 1):
        g = tuple(tokens[i : i + n])
        grams[g] = grams.get(g, 0) + 1
    if not grams:
        return 0.0
    top = max(grams.values())
    total = len(tokens) - n + 1
    return top / total if total else 0.0


def assess_body_quality(
    text: str,
    *,
    profile: BodyQualityProfile = "scholarly",
) -> BodyQualityReport:
    """Assess body text quality for preprocess diagnostics.

    Never sets import_eligible. Scholarly short text is soft-signal only.
    """
    if profile not in ("web", "scholarly"):
        raise ValueError(f"unknown body quality profile: {profile!r}")

    stripped = text.strip()
    if not stripped:
        return BodyQualityReport(
            profile=profile,
            status="empty",
            word_count=0,
            scores={"mean_word_len": 0.0, "symbol_ratio": 0.0, "ngram_repetition": 0.0},
            rule_hits=("empty",),
        )

    tokens = _tokens(stripped)
    word_count = len(tokens)
    mean_len = _mean_word_len(tokens)
    sym_ratio = _symbol_ratio(stripped, word_count)
    ngram_share = _ngram_repetition_share(tokens)
    scores = {
        "mean_word_len": round(mean_len, 4),
        "symbol_ratio": round(sym_ratio, 4),
        "ngram_repetition": round(ngram_share, 4),
    }
    hits: list[str] = []

    if profile == "web" and word_count < _WEB_MIN_WORDS:
        hits.append("too_short")
    elif profile == "scholarly" and word_count < _SCHOLARLY_MIN_WORDS:
        hits.append("very_short_soft")

    if word_count and (mean_len < _MIN_MEAN_WORD_LEN or mean_len > _MAX_MEAN_WORD_LEN):
        hits.append("mean_word_len_outlier")
    if word_count and sym_ratio > _MAX_SYMBOL_RATIO:
        hits.append("high_symbol_ratio")
    if ngram_share > _NGRAM_REPEAT_SHARE:
        hits.append("ngram_repetition")

    if not hits:
        status = "ok"
    elif profile == "web" and ("too_short" in hits or "ngram_repetition" in hits):
        status = "low_quality" if "too_short" not in hits else "too_short"
    elif profile == "scholarly" and hits == ["very_short_soft"]:
        status = "soft_signal"
    else:
        status = "soft_signal" if profile == "scholarly" else "low_quality"

    return BodyQualityReport(
        profile=profile,
        status=status,
        word_count=word_count,
        scores=scores,
        rule_hits=tuple(hits),
    )


__all__ = [
    "BodyQualityProfile",
    "BodyQualityReport",
    "assess_body_quality",
]
