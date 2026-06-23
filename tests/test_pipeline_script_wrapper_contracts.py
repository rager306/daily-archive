from __future__ import annotations

import ast
import importlib.util
import json
from pathlib import Path

from research_graph.application.pipeline_script_inventory import (
    ScriptClassification,
    ScriptInventory,
    validate_inventory,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = REPO_ROOT / "data" / "pipeline-script-architecture" / "script-inventory.json"
AUDIT_SCRIPT = REPO_ROOT / "scripts" / "audit_pipeline_scripts.py"


def _load_inventory() -> ScriptInventory:
    return ScriptInventory.from_dict(json.loads(INVENTORY_PATH.read_text()))


def _load_audit_module():
    spec = importlib.util.spec_from_file_location("audit_pipeline_scripts", AUDIT_SCRIPT)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _literal_string(node: ast.AST) -> str | None:
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        return node.value
    return None


def _declared_cli_flags(script_path: Path) -> set[str]:
    tree = ast.parse(script_path.read_text())
    flags: set[str] = set()
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        if not isinstance(func, ast.Attribute) or func.attr != "add_argument":
            continue
        for arg in node.args:
            value = _literal_string(arg)
            if value and value.startswith("-"):
                flags.add(value)
    return flags


def test_generated_inventory_is_current_with_audit_builder() -> None:
    module = _load_audit_module()
    generated = _load_inventory()
    rebuilt = module.build_inventory(REPO_ROOT)

    assert validate_inventory(generated) == []
    assert generated.to_dict() == rebuilt.to_dict()


def test_all_inventory_scripts_exist_and_are_entrypoints() -> None:
    inventory = _load_inventory()

    for item in inventory.items:
        script_path = REPO_ROOT / item.path
        assert script_path.exists(), f"{item.script_id}: missing script path {item.path}"
        source = script_path.read_text()
        assert "if __name__ == \"__main__\"" in source or "if __name__ == '__main__'" in source, (
            f"{item.script_id}: script must keep a CLI entrypoint guard"
        )


def test_required_cli_flags_are_declared_by_wrapper_scripts() -> None:
    inventory = _load_inventory()

    for item in inventory.items:
        required_flags = set(item.contract.required_flags)
        if not required_flags:
            continue
        declared_flags = _declared_cli_flags(REPO_ROOT / item.path)
        missing = sorted(required_flags - declared_flags)
        assert not missing, f"{item.script_id}: missing required CLI flags {missing}"


def test_production_candidates_have_migration_contracts() -> None:
    inventory = _load_inventory()

    for item in inventory.items:
        if item.classification is not ScriptClassification.PRODUCTION_CANDIDATE:
            continue
        assert item.migration_slice, f"{item.script_id}: missing migration_slice"
        assert item.contract.inputs, f"{item.script_id}: missing contract inputs"
        assert item.contract.expected_outputs, f"{item.script_id}: missing expected outputs"
        assert item.contract.verification, f"{item.script_id}: missing verification command"
        assert item.contract.summary_fields, f"{item.script_id}: missing summary fields"


def test_contract_output_paths_are_grouped_by_migration_slice() -> None:
    inventory = _load_inventory()
    outputs_by_slice: dict[str, list[str]] = {}

    for item in inventory.items:
        outputs_by_slice.setdefault(str(item.migration_slice), []).extend(
            item.contract.expected_outputs
        )

    assert "data/r024-218-document-corpus-v1/parser-chunking/summary.json" in outputs_by_slice["S03"]
    assert "data/r024-218-document-corpus-v1/networkx-probe/summary.json" in outputs_by_slice["S05"]
    assert "data/r024-53-document-corpus-v1/quality-metrics.json" in outputs_by_slice["S04"]
    assert "data/article_catalog/article_catalog/index.json" in outputs_by_slice["S02"]


def test_quality_metrics_contracts_remain_visible_to_s04() -> None:
    inventory = _load_inventory()
    quality_items = [item for item in inventory.items if item.category.value == "quality-metrics"]

    assert len(quality_items) == 4
    assert {item.migration_slice for item in quality_items} == {"S04"}
    assert all("quality" in item.path for item in quality_items)
    assert all(
        any(output.endswith("quality-metrics.json") for output in item.contract.expected_outputs)
        for item in quality_items
    )


def _imported_and_called_names(script_path: Path) -> tuple[set[str], set[str]]:
    tree = ast.parse(script_path.read_text())
    imported_names = {
        alias.name
        for node in ast.walk(tree)
        if isinstance(node, ast.ImportFrom)
        for alias in node.names
    }
    called_names = {
        node.func.id
        for node in ast.walk(tree)
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
    }
    return imported_names, called_names


def test_canonical_ingest_cli_uses_application_wrapper_not_legacy_orchestration() -> None:
    imported_names, called_names = _imported_and_called_names(
        REPO_ROOT / "scripts" / "ingest_to_canonical_catalog.py"
    )

    assert "CatalogIngestUseCase" in imported_names
    assert "M061SourceAssetStore" in imported_names
    assert "FilesystemCatalogRepository" in imported_names
    assert "ingest_catalog" not in imported_names
    assert "ingest_catalog" not in called_names


def test_m056_ingest_cli_uses_application_wrapper_not_legacy_orchestration() -> None:
    imported_names, called_names = _imported_and_called_names(
        REPO_ROOT / "scripts" / "ingest_m056_corpus.py"
    )

    assert "CatalogIngestUseCase" in imported_names
    assert "M056CumulativeCorpusSourceAssetStore" in imported_names
    assert "M056FilesystemCatalogRepository" in imported_names
    assert "load_m056_corpus" not in imported_names
    assert "verify_m056_sha256" not in imported_names
    assert "build_article_record" not in imported_names
    assert "write_article_record" not in imported_names
    assert "load_m056_corpus" not in called_names
    assert "verify_m056_sha256" not in called_names


def test_parser_replay_scripts_use_application_wrapper_not_parser_internals() -> None:
    scripts = [
        "replay_r024_10_document_parser_chunking.py",
        "replay_r024_20_document_parser_chunking.py",
        "replay_r024_53_document_parser_chunking.py",
        "replay_r024_218_document_parser_chunking.py",
    ]
    forbidden = {
        "FullTextSource",
        "ingest_full_text",
        "parse_article",
        "build_page_index_from_parsed",
    }

    for script in scripts:
        imported_names, called_names = _imported_and_called_names(REPO_ROOT / "scripts" / script)
        assert "ParserReplayUseCase" in imported_names
        assert "FilesystemParserReplaySourceLoader" in imported_names
        assert "ExistingFullTextParserAdapter" in imported_names
        assert "PageIndexChunkWriterAdapter" in imported_names
        assert not forbidden & imported_names
        assert not forbidden & called_names


def test_coverage_report_script_uses_application_use_case_and_writer() -> None:
    imported_names, called_names = _imported_and_called_names(
        REPO_ROOT / "scripts" / "build_r024_coverage_report.py"
    )

    assert "CorpusCoverageUseCase" in imported_names
    assert "FilesystemCoverageReportWriter" in imported_names
    assert "CatalogCoverageInput" in imported_names
    assert "ParserCoverageInput" in imported_names
    assert "GraphProbeCoverageInput" in imported_names
    assert "CorpusCoverageUseCase" in called_names
    assert "FilesystemCoverageReportWriter" in called_names


def test_networkx_probe_scripts_use_application_use_case_and_adapter() -> None:
    scripts = [
        "build_r024_networkx_probe.py",
        "build_r024_20_document_networkx_probe.py",
        "build_r024_53_document_networkx_probe.py",
        "build_r024_218_document_networkx_probe.py",
    ]

    for script in scripts:
        imported_names, called_names = _imported_and_called_names(REPO_ROOT / "scripts" / script)
        assert "GraphProbeUseCase" in imported_names
        assert "NetworkXGraphProbeAdapter" in imported_names
        assert "R024NetworkXProbeConfig" in imported_names
        assert "build_request" in imported_names
        assert "write_legacy_artifacts" in imported_names
        assert "GraphProbeUseCase" in called_names
        assert "NetworkXGraphProbeAdapter" in called_names
        assert "R024NetworkXProbeConfig" in called_names
        assert "build_request" in called_names
        assert "write_legacy_artifacts" in called_names
