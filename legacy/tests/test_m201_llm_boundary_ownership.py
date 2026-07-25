"""M201 S06: LLM boundary ownership ratchets (semantic vs artifact)."""

from __future__ import annotations

import ast
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
APP = REPO / "src" / "research_graph" / "application"
ARTIFACTS = REPO / "src" / "research_graph" / "infrastructure" / "papers" / "artifacts"
LLM = REPO / "src" / "research_graph" / "infrastructure" / "llm"

FORBIDDEN_IN_ARTIFACTS = {
    "research_graph.application.paper_extraction",
    "research_graph.application.chunk_extraction",
    "research_graph.application.extraction_pilot",
}
FORBIDDEN_IN_APP_EXTRACTION = {
    "research_graph.infrastructure.papers.artifacts.minimax_boundary",
    "research_graph.infrastructure.papers.artifacts.metrics",
}


def _imports(path: Path) -> set[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    mods: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                mods.add(alias.name)
        elif isinstance(node, ast.ImportFrom) and node.module:
            mods.add(node.module)
    return mods


def test_artifact_modules_do_not_import_semantic_extraction_use_cases() -> None:
    violations: list[str] = []
    for path in ARTIFACTS.rglob("*.py"):
        if path.name == "__init__.py":
            continue
        for mod in _imports(path):
            if mod in FORBIDDEN_IN_ARTIFACTS or any(
                mod.startswith(f"{f}.") for f in FORBIDDEN_IN_ARTIFACTS
            ):
                violations.append(f"{path.relative_to(REPO)} imports {mod}")
    assert not violations, violations


def test_application_extraction_does_not_import_artifact_minimax_boundary() -> None:
    files = [
        APP / "paper_extraction.py",
        APP / "chunk_extraction.py",
        APP / "extraction_pilot.py",
    ]
    violations: list[str] = []
    for path in files:
        assert path.is_file(), path
        for mod in _imports(path):
            if mod in FORBIDDEN_IN_APP_EXTRACTION or any(
                mod.startswith(f"{f}.") for f in FORBIDDEN_IN_APP_EXTRACTION
            ):
                violations.append(f"{path.name} imports {mod}")
    assert not violations, violations


def test_minimax_and_glm_clients_expose_extract() -> None:
    for name in ("minimax_client.py", "glm_client.py", "fallback_client.py"):
        path = LLM / name
        tree = ast.parse(path.read_text(encoding="utf-8"))
        methods: set[str] = set()
        for node in ast.walk(tree):
            if isinstance(node, ast.FunctionDef) and node.name == "extract":
                methods.add(node.name)
        assert "extract" in methods, f"{name} must define extract()"


def test_extraction_schemas_are_not_artifact_schemas() -> None:
    text = (LLM / "extraction_schemas.py").read_text(encoding="utf-8")
    assert "extract_entities" in text
    assert "article_artifact" not in text
    assert "minimax_hint" not in text
