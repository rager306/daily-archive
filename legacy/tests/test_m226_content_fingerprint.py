"""M226 S01: content fingerprint over cleaned body text."""

from __future__ import annotations

import hashlib

import pytest

from research_graph.application.corpus.content_fingerprint import (
    ContentFingerprint,
    fingerprint_cleaned_body,
)


def test_empty_fingerprint_stable() -> None:
    result = fingerprint_cleaned_body("")
    assert result.sha256 == hashlib.sha256(b"").hexdigest()
    assert result.char_count == 0
    assert result.import_eligible is False
    assert result.graph_writes_allowed is False


def test_same_text_same_hash() -> None:
    text = "Graph neural networks pass messages along edges."
    a = fingerprint_cleaned_body(text)
    b = fingerprint_cleaned_body(text)
    assert a.sha256 == b.sha256
    assert a.char_count == len(text)
    assert a.sha256 == hashlib.sha256(text.encode("utf-8")).hexdigest()


def test_different_text_different_hash() -> None:
    a = fingerprint_cleaned_body("alpha")
    b = fingerprint_cleaned_body("beta")
    assert a.sha256 != b.sha256


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        ContentFingerprint(sha256="abc", char_count=0, import_eligible=True)
