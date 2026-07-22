"""M200 S05/S06: runtime ownership ratchets for canonical day/source analysis."""

from __future__ import annotations

import ast
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
CLI_PATH = REPO / "src" / "research_graph" / "cli" / "__init__.py"
APPLICATION = REPO / "src" / "research_graph" / "application"


def _cli_tree() -> ast.Module:
    return ast.parse(CLI_PATH.read_text(encoding="utf-8"))


def test_cli_run_analysis_async_delegates_to_analyze_day_use_case() -> None:
    tree = _cli_tree()
    run_fn = None
    for node in tree.body:
        if isinstance(node, ast.AsyncFunctionDef) and node.name == "run_analysis_async":
            run_fn = node
            break
    assert run_fn is not None, "run_analysis_async must exist"

    names: set[str] = set()
    for node in ast.walk(run_fn):
        if isinstance(node, ast.Name):
            names.add(node.id)
        elif isinstance(node, ast.Attribute):
            names.add(node.attr)
    assert "AnalyzeDayUseCase" in names
    # Must not re-implement day orchestration helpers inside CLI.
    assert "fetch_papers" not in names or "AnalyzeDayUseCase" in names


def test_cli_has_no_direct_day_scoring_helpers() -> None:
    source = CLI_PATH.read_text(encoding="utf-8")
    assert "async def _process_paper_async" not in source
    assert "async def _score_papers_bounded" not in source
    tree = _cli_tree()
    func_names = {
        node.name
        for node in tree.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    }
    assert "_process_paper_async" not in func_names
    assert "_score_papers_bounded" not in func_names


def test_single_paper_pipeline_composition_root() -> None:
    """Only profiles/paper.py may define build_*_paper_pipeline composition roots."""
    roots: list[str] = []
    for path in APPLICATION.rglob("*.py"):
        if path.name == "test_architecture_inventory.py":
            continue
        text = path.read_text(encoding="utf-8")
        tree = ast.parse(text)
        for node in tree.body:
            if isinstance(node, ast.FunctionDef) and node.name.endswith("_paper_pipeline"):
                rel = path.relative_to(REPO).as_posix()
                roots.append(f"{rel}:{node.name}")
    assert roots, "expected at least one paper pipeline builder"
    allowed = {
        "src/research_graph/application/profiles/paper.py:build_paper_pipeline",
        "src/research_graph/application/profiles/paper.py:build_wired_paper_pipeline",
    }
    unexpected = [r for r in roots if r not in allowed]
    assert not unexpected, f"extra paper composition roots: {unexpected}"


def test_analyze_source_and_analyze_day_live_in_application() -> None:
    assert (APPLICATION / "analyze_source.py").is_file()
    assert (APPLICATION / "analyze_day.py").is_file()
    assert (APPLICATION / "shadow_parity.py").is_file()


def test_cli_does_not_import_build_wired_paper_pipeline() -> None:
    """CLI must not become a second composition root for paper pipelines."""
    source = CLI_PATH.read_text(encoding="utf-8")
    assert "build_wired_paper_pipeline" not in source
    assert "build_paper_pipeline" not in source


def _function_names_used(fn: ast.AST) -> set[str]:
    names: set[str] = set()
    for node in ast.walk(fn):
        if isinstance(node, ast.Name):
            names.add(node.id)
        elif isinstance(node, ast.Attribute):
            names.add(node.attr)
    return names


def test_wrapper_commands_delegate_to_run_analysis_async() -> None:
    tree = _cli_tree()
    by_name: dict[str, ast.AST] = {}
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            by_name[node.name] = node
    assert "run_analysis" in by_name
    assert "run_command_async" in by_name
    assert "run_analysis_async" in _function_names_used(by_name["run_analysis"])
    assert "run_analysis_async" in _function_names_used(by_name["run_command_async"])
    # Wrappers must not construct ArxivClient themselves
    assert "ArxivClient" not in _function_names_used(by_name["run_analysis"])
    assert "ArxivClient" not in _function_names_used(by_name["run_command_async"])


def test_analyze_source_is_application_single_source_tracer() -> None:
    text = (APPLICATION / "analyze_source.py").read_text(encoding="utf-8")
    assert "class AnalyzeSourceUseCase" in text
    assert "build_wired_paper_pipeline" in text
    assert "PipelineOrchestrator" in text

