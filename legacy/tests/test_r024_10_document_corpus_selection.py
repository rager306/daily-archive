"""Tests for R024 10-document corpus selection."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
SELECTION = REPO_ROOT / "data" / "r024-10-document-corpus-v1" / "selection.json"
SCRIPT = REPO_ROOT / "scripts" / "verify_r024_10_document_corpus_selection.py"


def test_r024_selection_file_exists() -> None:
    assert SELECTION.exists(), f"selection.json not found: {SELECTION}"


def test_r024_selection_has_10_articles() -> None:
    sel = json.loads(SELECTION.read_text())
    assert len(sel["articles"]) == 10, f"expected 10 articles, got {len(sel['articles'])}"


def test_r024_selection_unique_keys() -> None:
    sel = json.loads(SELECTION.read_text())
    keys = [a["article_key"] for a in sel["articles"]]
    assert len(set(keys)) == len(keys), f"duplicate article_keys: {keys}"


def test_r024_selection_5_baseline_5_extension() -> None:
    sel = json.loads(SELECTION.read_text())
    baseline = [
        a
        for a in sel["articles"]
        if a.get("selection_source") == "m025-rlm-dspy-pageindex-smoke-v1"
    ]
    extension = [
        a for a in sel["articles"] if a.get("selection_source") == "r024-10-document-corpus-v1"
    ]
    assert len(baseline) == 5, f"expected 5 baseline, got {len(baseline)}"
    assert len(extension) == 5, f"expected 5 extension, got {len(extension)}"


def test_r024_selection_no_network() -> None:
    sel = json.loads(SELECTION.read_text())
    assert sel["network_policy"].get("test_phase_must_not_fetch") is True


def test_r024_selection_all_have_local_sources() -> None:
    sel = json.loads(SELECTION.read_text())
    for a in sel["articles"]:
        key = a["article_key"]
        found = list(
            REPO_ROOT.glob(f"data/article_catalog/article_catalog/**/{key}/loader/summary.json")
        )
        found2 = list(REPO_ROOT.glob(f"data/article_catalog/article_catalog/**/{key}/article.json"))
        assert found or found2, f"no local source for {a['article_ref']} (key={key})"


def test_r024_verifier_script_passes() -> None:
    result = subprocess.run(
        [sys.executable, str(SCRIPT)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode == 0, f"verifier failed: {result.stdout}\n{result.stderr}"
