"""Deterministic language detect for preprocess (M225 S01).

Lightweight heuristic: Unicode script share + stopword samples.
No external language-id dependency. Application pure; never authorizes import.

# ponytail: stopword+script heuristic, upgrade path: optional langdetect/fasttext
"""

from __future__ import annotations

import re
from dataclasses import dataclass

_WORD_RE = re.compile(r"[A-Za-z\u0400-\u04FF\u00C0-\u024F]+")

# Small function-word samples (not exhaustive).
_STOPWORDS: dict[str, frozenset[str]] = {
    "en": frozenset(
        "the and of to in a is that for on with as by this are from be or an".split()
    ),
    "de": frozenset(
        "der die und das ist von zu den mit nicht ein eine auf für".split()
    ),
    "fr": frozenset(
        "le de et la les des un une est du en dans que qui pour".split()
    ),
    "ru": frozenset(
        "и в не на что с по это как а то все она они из за для".split()
    ),
}


@dataclass(frozen=True, slots=True)
class LanguageDetectResult:
    """Language guess for body text. Always import-blocked."""

    language: str
    confidence: float
    method: str = "script_stopword_v1"
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("language detect cannot authorize import/writes")


def _cyrillic_ratio(text: str) -> float:
    letters = [c for c in text if c.isalpha()]
    if not letters:
        return 0.0
    cyr = sum(1 for c in letters if "\u0400" <= c <= "\u04FF")
    return cyr / len(letters)


def _stopword_scores(tokens: list[str]) -> dict[str, float]:
    if not tokens:
        return dict.fromkeys(_STOPWORDS, 0.0)
    total = len(tokens)
    scores: dict[str, float] = {}
    for code, words in _STOPWORDS.items():
        hits = sum(1 for t in tokens if t in words)
        scores[code] = hits / total
    return scores


def detect_text_language(text: str) -> LanguageDetectResult:
    """Detect language of body text. Empty → unknown."""
    stripped = text.strip()
    if not stripped:
        return LanguageDetectResult(language="unknown", confidence=0.0)

    cyr = _cyrillic_ratio(stripped)
    if cyr >= 0.35:
        # Prefer Russian for Cyrillic-heavy scholarly text.
        tokens = [m.group(0).casefold() for m in _WORD_RE.finditer(stripped)]
        ru_score = _stopword_scores(tokens).get("ru", 0.0)
        conf = min(0.99, 0.55 + cyr * 0.35 + ru_score)
        return LanguageDetectResult(language="ru", confidence=round(conf, 4))

    tokens = [m.group(0).casefold() for m in _WORD_RE.finditer(stripped)]
    if len(tokens) < 3:
        return LanguageDetectResult(language="unknown", confidence=0.1)

    scores = _stopword_scores(tokens)
    # Latin languages only among stopword scores for non-Cyrillic.
    latin = {k: v for k, v in scores.items() if k != "ru"}
    best_lang = max(latin, key=lambda code: latin[code])
    best = latin[best_lang]
    if best <= 0.0 and len(tokens) >= 8:
        # Default scholarly English when enough Latin tokens, no stopword hits.
        return LanguageDetectResult(language="en", confidence=0.35)
    if best < 0.02:
        return LanguageDetectResult(language="unknown", confidence=round(best, 4))

    conf = min(0.95, 0.35 + best * 4.0)
    return LanguageDetectResult(language=best_lang, confidence=round(conf, 4))


__all__ = ["LanguageDetectResult", "detect_text_language"]
