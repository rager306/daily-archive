"""Content fingerprint for cleaned body custody context (M226 S01).

Deterministic SHA256 over UTF-8 body text. Application pure; never
authorizes import. Used for version/custody identity, not graph truth.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class ContentFingerprint:
    """Stable body fingerprint. Always import-blocked."""

    sha256: str
    char_count: int
    algorithm: str = "sha256"
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("content fingerprint cannot authorize import/writes")


def fingerprint_cleaned_body(text: str) -> ContentFingerprint:
    """SHA256 hex digest of UTF-8 body text (empty string is valid input)."""
    payload = text.encode("utf-8")
    return ContentFingerprint(
        sha256=hashlib.sha256(payload).hexdigest(),
        char_count=len(text),
    )


__all__ = ["ContentFingerprint", "fingerprint_cleaned_body"]
