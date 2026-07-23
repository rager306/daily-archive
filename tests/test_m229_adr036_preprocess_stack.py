"""M229: contract tests for ADR-036 non-LLM article preprocess stack."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADR_PATH = ROOT / "doc" / "adr" / "ADR-036-non-llm-article-preprocess-stack.md"
ADR_INDEX = ROOT / "doc" / "adr" / "ADR-INDEX.md"

REQUIRED_MODULES = [
    "application/corpus/body_text_clean.py",
    "application/corpus/body_quality.py",
    "application/corpus/html_main_content.py",
    "application/corpus/article_preprocess.py",
    "application/corpus/language_detect.py",
    "application/corpus/outline_signals.py",
    "application/corpus/content_fingerprint.py",
    "application/corpus/keyword_spans.py",
    "application/corpus/term_dense_window.py",
    "application/corpus/preprocess_summary.py",
]


def test_adr036_file_exists_and_accepted() -> None:
    assert ADR_PATH.is_file()
    text = ADR_PATH.read_text(encoding="utf-8")
    assert text.startswith("# ADR-036: Non-LLM Article Preprocess Stack")
    assert "Accepted (binding)" in text
    assert "import" in text.casefold()


def test_adr036_non_authorization_language() -> None:
    text = ADR_PATH.read_text(encoding="utf-8")
    lower = text.casefold()
    assert "we will not" in lower
    assert "import_eligible" in lower or "graph import" in lower
    assert "hybrid_claimed_success" in lower or "hybrid tei" in lower
    assert "does not authorize" in lower


def test_adr036_lists_key_modules() -> None:
    text = ADR_PATH.read_text(encoding="utf-8")
    for rel in REQUIRED_MODULES:
        assert rel in text, f"missing module path in ADR: {rel}"
        assert (ROOT / "src" / "research_graph" / rel).is_file(), rel


def test_adr_index_lists_adr036() -> None:
    index = ADR_INDEX.read_text(encoding="utf-8")
    assert "ADR-036" in index
    assert "Non-LLM Article Preprocess Stack" in index
    assert "ADR-036-non-llm-article-preprocess-stack.md" in index
    assert "Project-level ADR count: 36" in index


def test_yake_not_in_application_corpus() -> None:
    corpus = ROOT / "src" / "research_graph" / "application" / "corpus"
    for path in corpus.glob("*.py"):
        src = path.read_text(encoding="utf-8")
        assert "import yake" not in src
        assert "from yake" not in src
