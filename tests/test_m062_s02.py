from __future__ import annotations

import re
from pathlib import Path

from research_graph.infrastructure.retrieval.embedder import (
    DEFAULT_DIMENSIONS,
    DEFAULT_ENDPOINT,
    SAFETY_DEFAULTS,
)

ROOT = Path(__file__).resolve().parents[1]
ADR_PATH = ROOT / "doc/adr/ADR-019-fd-embedding-service-contract.md"
ADR_INDEX_PATH = ROOT / "doc/adr/ADR-INDEX.md"
CODEBASE_MEMORY_ADR_PATH = ROOT / ".codebase-memory/adr.md"
FORBIDDEN_HOST = "local" "host"

EXPECTED_ERROR_CODES = {
    "input_required",
    "input_too_long",
    "batch_too_large",
    "dimensions_invalid",
    "dimensions_required",
    "invalid_json",
    "unauthorized",
    "not_found",
    "method_not_allowed",
    "payload_too_large",
    "rate_limit_exceeded",
    "internal_error",
    "model_not_loaded",
    "model_overloaded",
    "shutting_down",
    "request_timeout",
}


def _adr_text() -> str:
    return ADR_PATH.read_text()


def test_adr_019_exists() -> None:
    assert ADR_PATH.exists()
    text = _adr_text()
    assert text.startswith("# ADR-019: M062 fd Embedding Service Contract")
    assert "**Status:** Accepted (binding)" in text
    assert FORBIDDEN_HOST not in text


def test_adr_019_full_m034_template() -> None:
    headings = re.findall(r"^## (\d+)\. ", _adr_text(), flags=re.MULTILINE)
    assert headings == [str(index) for index in range(15)]
    assert "## 14. LLM Reading Notes" in _adr_text()
    assert _adr_text().count("```mermaid") >= 2


def test_adr_019_references_fd_v2_md() -> None:
    text = _adr_text()
    assert "/root/fd-v2.md" in text
    assert "authoritative fd v2 service contract" in text
    assert "binding: yes" in text


def test_adr_includes_error_catalog() -> None:
    text = _adr_text()
    codes = set(re.findall(r"^\| \d+ \| `([^`]+)` \|", text, flags=re.MULTILINE))
    assert codes == EXPECTED_ERROR_CODES
    assert len(codes) == 16
    assert "error.code" in text
    assert "error.type" in text


def test_adr_includes_openapi_sketch() -> None:
    text = _adr_text()
    assert "## 6. OpenAPI 3.1 Sketch" in text
    for required in ("openapi: 3.1.0", "/v1/embeddings", "/health", "/live", "/ready", "/metrics"):
        assert required in text
    for required in ("post:", "get:", "schema:", "properties:", "securitySchemes"):
        assert required in text


def test_adr_includes_45_test_cases_reference() -> None:
    text = _adr_text()
    assert "45 fd v2 validation cases" in text
    assert "T-H-1" in text
    assert "T-E-15" in text
    assert "T-HDR-10" in text
    assert "T-P-5" in text
    assert "### 5.5 Endpoints existence (5 tests)" in text


def test_adr_index_updated() -> None:
    index_text = ADR_INDEX_PATH.read_text()
    assert "| ADR-019 | Accepted (binding) | M062 fd Embedding Service Contract | `doc/adr/ADR-019-fd-embedding-service-contract.md` |" in index_text
    assert "| ADR-018 | Accepted (binding) | M061 2-hop Evidence and M064 Trigger Evaluation |" in index_text


def test_codebase_memory_synced_and_m062_s01_regression() -> None:
    memory_text = CODEBASE_MEMORY_ADR_PATH.read_text()
    assert "| ADR-019 | Accepted (binding) | `doc/adr/ADR-019-fd-embedding-service-contract.md` | M062 fd Embedding Service Contract |" in memory_text
    assert DEFAULT_ENDPOINT == "http://127.0.0.1:8000/v1/embeddings"
    assert DEFAULT_DIMENSIONS == 1024
    assert SAFETY_DEFAULTS == {
        "graph_writes_authorized": False,
        "production_import_authorized": False,
        "fact_promotion_authorized": False,
        "external_network_authorized": False,
        "llm_calls_authorized": False,
    }
