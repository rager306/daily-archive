from __future__ import annotations

import ast
import importlib.util
from pathlib import Path

from research_graph.domain.ports import ProjectionRequest
from research_graph.infrastructure.graph.projection_backends import (
    DisabledFalkorProjectionAdapter,
    DisabledLadybugProjectionAdapter,
)
from research_graph.workflows.universal_kb.contracts import CandidatePacket

ROOT = Path(__file__).resolve().parents[1]
NO_WRITE_SOURCE_PATHS = (
    ROOT / "src/research_graph/workflows/universal_kb/rehearsal.py",
    ROOT / "src/research_graph/domain/graph_projection_schema.py",
    ROOT / "src/research_graph/infrastructure/graph/networkx_probe.py",
    ROOT / "src/research_graph/infrastructure/graph/projection_backends.py",
)


def _python_files(*roots: str) -> list[Path]:
    files: list[Path] = []
    for root in roots:
        base = ROOT / root
        if base.exists():
            files.extend(path for path in base.rglob("*.py") if path.name != Path(__file__).name)
    return files


def test_retired_graph_readiness_command_and_shim_are_not_restored() -> None:
    retired_module = ".".join(("arxiv_archive", "graph_readiness_review"))
    retired_command = f"python -m {retired_module}"

    assert not (ROOT / "src/arxiv_archive/graph_readiness_review.py").exists()
    try:
        spec = importlib.util.find_spec(retired_module)
    except ModuleNotFoundError:
        spec = None
    assert spec is None

    offenders = []
    for path in _python_files("src", "scripts", "tests"):
        text = path.read_text(encoding="utf-8")
        if retired_command in text or retired_module in text:
            offenders.append(path.relative_to(ROOT).as_posix())
    assert offenders == []


def test_no_write_projection_path_has_no_backend_db_imports_or_write_calls() -> None:
    forbidden_import_terms = ("ladybug", "falkor", "ladybug_client")
    forbidden_call_names = {
        "connect",
        "init_schema",
        "import_graph",
        "persist_graph",
        "promote_import",
        "upsert_scientific_kg",
        "write_graph",
        "write_to_graph",
    }
    offenders: list[tuple[str, str, str]] = []

    for path in NO_WRITE_SOURCE_PATHS:
        tree = ast.parse(path.read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    if any(term in alias.name.lower() for term in forbidden_import_terms):
                        offenders.append((path.relative_to(ROOT).as_posix(), "import", alias.name))
            elif isinstance(node, ast.ImportFrom):
                module = node.module or ""
                if any(term in module.lower() for term in forbidden_import_terms):
                    offenders.append((path.relative_to(ROOT).as_posix(), "from", module))
            elif isinstance(node, ast.Call):
                name = ""
                if isinstance(node.func, ast.Attribute):
                    name = node.func.attr
                elif isinstance(node.func, ast.Name):
                    name = node.func.id
                if name in forbidden_call_names:
                    offenders.append((path.relative_to(ROOT).as_posix(), "call", name))
    assert offenders == []


def test_no_write_projection_source_never_sets_write_or_import_flags_true() -> None:
    flags = (
        "graph_import_allowed",
        "graph_write_allowed",
        "graphdb_written",
        "import_eligible",
        "ladybugdb_written",
        "production_import_attempted",
        "promotion_allowed",
    )
    offenders = []
    for path in NO_WRITE_SOURCE_PATHS:
        text = path.read_text(encoding="utf-8")
        for flag in flags:
            if f"{flag}=True" in text or f"{flag} = True" in text:
                offenders.append((path.relative_to(ROOT).as_posix(), flag))
    assert offenders == []


def test_disabled_backend_seams_remain_no_write_and_not_import_eligible() -> None:
    candidate = CandidatePacket(
        candidate_id="candidate-1",
        evidence_refs=("artifact:evidence-1",),
        candidate_type="graph_candidate",
        graph_node_refs=("node:paper:1",),
        graph_edge_refs=("edge:paper:1->claim:1",),
        provenance_refs=("source:fixture",),
    )
    request = ProjectionRequest(candidate_packet=candidate)

    for adapter in (DisabledLadybugProjectionAdapter(), DisabledFalkorProjectionAdapter()):
        result = adapter.project(request)
        result.assert_no_write()
        assert result.safety_flags.import_eligible is False
        assert result.safety_flags.graphdb_written is False
        assert result.diagnostics[0].code == "backend_projection_disabled"


def test_recent_m195_scope_artifacts_keep_readiness_disclaimers() -> None:
    artifacts = (
        ROOT / "data/architecture-assessment/m195-s10-scope-verification.md",
        ROOT / "data/architecture-assessment/m195-s11-scope-verification.md",
        ROOT / "data/architecture-assessment/m195-s12-scope-verification.md",
    )
    for path in artifacts:
        text = path.read_text(encoding="utf-8").lower()
        assert "not readiness evidence" in text or "not production graph readiness" in text
