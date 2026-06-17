from __future__ import annotations

import ast
from pathlib import Path

ADR_005 = "doc/adr/m034/ADR-005-no-direct-extractor-to-graphdb-path.md"

GUARDED_MODULES = [
    Path("src/arxiv_archive/universal_kb_sidecar_boundary.py"),
    Path("src/arxiv_archive/universal_kb_review_assistance.py"),
    Path("src/arxiv_archive/universal_kb_substrate_rehearsal.py"),
    Path("src/arxiv_archive/minimax_structured.py"),
    Path("src/research_graph/papers/artifacts/minimax_boundary.py"),
]

READ_ONLY_EXEMPTIONS = {
    Path("src/arxiv_archive/hybrid_retrieval.py"),
    Path("src/arxiv_archive/graph_readiness_review.py"),
}

FORBIDDEN_IMPORT_PREFIXES = (
    "ladybugdb",
    "falkordb",
    "helixdb",
    "graphdb",
    "neo4j",
)

FORBIDDEN_AUTHORITY_PATTERNS = (
    "helper_evidence_only=False",
    "helper_evidence_only = False",
    "minimax_source_of_truth=True",
    "minimax_source_of_truth = True",
    "raw_prompt_persisted=True",
    "raw_prompt_persisted = True",
    "credential_value_logged=True",
    "credential_value_logged = True",
    "graph_write_allowed=True",
    "graph_write_allowed = True",
    "promotion_allowed=True",
    "promotion_allowed = True",
    "production_import_attempted=True",
    "production_import_attempted = True",
    "import_eligible=True",
    "import_eligible = True",
)

FORBIDDEN_CALL_NAMES = {
    "write_graph",
    "write_to_graph",
    "write_to_graphdb",
    "write_to_ladybugdb",
    "promote_candidate",
    "promote_to_fact",
    "production_import",
    "import_to_graph",
}


def _imports(path: Path) -> list[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    names: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            names.extend(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module:
            names.append(node.module)
    return names


def _call_names(path: Path) -> list[str]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    names: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Call):
            func = node.func
            if isinstance(func, ast.Name):
                names.append(func.id)
            elif isinstance(func, ast.Attribute):
                names.append(func.attr)
    return names


def test_adr_005_exists_and_is_binding() -> None:
    text = Path(ADR_005).read_text(encoding="utf-8")

    assert "No Direct Extractor to GraphDB Path" in text
    assert "Binding Level:** binding" in text
    assert "will not allow direct parser/extractor/LLM/sidecar writes" in text


def test_guarded_modules_do_not_import_graph_write_libraries() -> None:
    for path in GUARDED_MODULES:
        imported = _imports(path)
        offenders = [
            module
            for module in imported
            if module.lower().startswith(FORBIDDEN_IMPORT_PREFIXES)
        ]
        assert not offenders, f"{path} violates ADR-005 with forbidden imports: {offenders}"


def test_guarded_modules_do_not_call_promotion_or_graph_write_functions() -> None:
    for path in GUARDED_MODULES:
        offenders = sorted(set(_call_names(path)) & FORBIDDEN_CALL_NAMES)
        assert not offenders, f"{path} violates ADR-005 with forbidden calls: {offenders}"


def test_guarded_modules_do_not_set_llm_or_rehearsal_authority_true() -> None:
    for path in GUARDED_MODULES:
        compact = path.read_text(encoding="utf-8").replace("\n", "")
        offenders = [pattern for pattern in FORBIDDEN_AUTHORITY_PATTERNS if pattern in compact]
        assert not offenders, f"{path} grants forbidden helper/write authority: {offenders}"


def test_read_only_graph_modules_are_explicitly_exempt_not_promotion_paths() -> None:
    for path in READ_ONLY_EXEMPTIONS:
        text = path.read_text(encoding="utf-8").lower()
        assert path.exists()
        assert "write_to_graphdb" not in text
        assert "promote_to_fact" not in text
        assert "production_import(" not in text
