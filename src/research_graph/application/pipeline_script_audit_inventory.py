"""Explicit recurring pipeline script inventory builder."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.pipeline_script_inventory import (
    ScriptCategory,
    ScriptClassification,
    ScriptContract,
    ScriptInventory,
    ScriptInventoryItem,
    ValidationIssue,
    validate_inventory,
)


def _contract(
    *,
    inputs: list[str],
    expected_outputs: list[str],
    verification: list[str],
    required_flags: list[str] | None = None,
    summary_fields: list[str] | None = None,
) -> ScriptContract:
    return ScriptContract(
        inputs=inputs,
        expected_outputs=expected_outputs,
        verification=verification,
        required_flags=required_flags or [],
        summary_fields=summary_fields or [],
    )


def _item(
    *,
    script_id: str,
    path: str,
    category: ScriptCategory,
    classification: ScriptClassification,
    migration_slice: str,
    contract: ScriptContract,
    notes: str,
) -> ScriptInventoryItem:
    return ScriptInventoryItem(
        script_id=script_id,
        path=path,
        category=category,
        classification=classification,
        migration_slice=migration_slice,
        contract=contract,
        notes=notes,
    )


def build_inventory(repo_root: Path = Path(".")) -> ScriptInventory:
    """Build the explicit recurring pipeline script inventory."""

    repo_root = repo_root.resolve()
    del repo_root  # contracts are repo-relative and intentionally stable

    items = [
        _item(
            script_id="ingest-to-canonical-catalog",
            path="scripts/ingest_to_canonical_catalog.py",
            category=ScriptCategory.CATALOG_INGEST,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S02",
            contract=_contract(
                inputs=["artifacts/m061-2hop/summary.json"],
                expected_outputs=[
                    "data/article_catalog/article_catalog/index.json",
                    "artifacts/m061-2hop/s04-ingest-report.md",
                ],
                verification=["uv run pytest tests/test_catalog_ingest.py tests/test_ingest_cli.py"],
                required_flags=["--m061-root", "--catalog-root", "--report-path", "--no-index", "--no-network"],
                summary_fields=[
                    "processed_pdf_copies",
                    "unique_arxiv_ids",
                    "ingested",
                    "skipped",
                    "fallback",
                ],
            ),
            notes="Current canonical ingest CLI delegates to the M122 application use case with filesystem adapters while preserving M061 report compatibility.",
        ),
        _item(
            script_id="ingest-m056-corpus",
            path="scripts/ingest_m056_corpus.py",
            category=ScriptCategory.CATALOG_INGEST,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S02",
            contract=_contract(
                inputs=["data/m056-cumulative-corpus/cumulative-corpus.json"],
                expected_outputs=[
                    "data/r024-218-document-corpus-v1/catalog-ingest/ingest-summary.json",
                    "data/r024-218-document-corpus-v1/catalog-ingest/ingest-events.jsonl",
                ],
                verification=["uv run pytest tests/test_catalog_ingest_m056.py tests/test_catalog_expansion_m121.py"],
                summary_fields=["verified_pdf_count", "catalog_record_count", "ingested", "skipped"],
            ),
            notes="M121 ingest path with offline M056 metadata patching; migrate after the shared catalog ingest use case exists.",
        ),
        _item(
            script_id="m061-legacy-ingest",
            path="scripts/m061_ingest_to_canonical_catalog.py",
            category=ScriptCategory.CATALOG_INGEST,
            classification=ScriptClassification.COMPATIBILITY_WRAPPER,
            migration_slice="S02",
            contract=_contract(
                inputs=["artifacts/m061-2hop/summary.json"],
                expected_outputs=["artifacts/m061-2hop/s04-ingest-report.md"],
                verification=["uv run pytest tests/test_m061_legacy_delegate.py"],
                required_flags=["--m061-root", "--catalog-root", "--report-path", "--no-index", "--no-network"],
                summary_fields=["processed_pdf_copies", "unique_arxiv_ids", "ingested", "skipped"],
            ),
            notes="Historical compatibility entrypoint retained because project knowledge names its canonical ingest pattern.",
        ),
        _item(
            script_id="replay-r024-10-parser-chunking",
            path="scripts/replay_r024_10_document_parser_chunking.py",
            category=ScriptCategory.PARSER_REPLAY,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S03",
            contract=_contract(
                inputs=["data/r024-10-document-corpus-v1/selection.json"],
                expected_outputs=[
                    "data/r024-10-document-corpus-v1/parser-chunking/summary.json",
                    "data/r024-10-document-corpus-v1/parser-chunking/events.jsonl",
                ],
                verification=["uv run pytest tests/test_r024_parser_chunking.py"],
                summary_fields=["completed", "skipped", "errors", "chunks"],
            ),
            notes="Small parser replay variant; migrate after the 218-document path defines the shared use case.",
        ),
        _item(
            script_id="replay-r024-20-parser-chunking",
            path="scripts/replay_r024_20_document_parser_chunking.py",
            category=ScriptCategory.PARSER_REPLAY,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S03",
            contract=_contract(
                inputs=["data/r024-20-document-corpus-v1/selection.json"],
                expected_outputs=[
                    "data/r024-20-document-corpus-v1/parser-chunking/summary.json",
                    "data/r024-20-document-corpus-v1/parser-chunking/events.jsonl",
                ],
                verification=["uv run pytest tests/test_r024_20_document_parser_chunking.py"],
                summary_fields=["completed", "skipped", "errors", "chunks"],
            ),
            notes="20-document replay variant; preserve output fields while sharing S03 application logic.",
        ),
        _item(
            script_id="replay-r024-53-parser-chunking",
            path="scripts/replay_r024_53_document_parser_chunking.py",
            category=ScriptCategory.PARSER_REPLAY,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S03",
            contract=_contract(
                inputs=["data/r024-53-document-corpus-v1/selection.json"],
                expected_outputs=[
                    "data/r024-53-document-corpus-v1/parser-chunking/summary.json",
                    "data/r024-53-document-corpus-v1/parser-chunking/events.jsonl",
                ],
                verification=["uv run pytest tests/test_r024_53_document_parser_chunking.py"],
                summary_fields=["completed", "skipped", "errors", "chunks"],
            ),
            notes="53-document replay variant; preserve source resolution semantics through the shared S03 use case.",
        ),
        _item(
            script_id="replay-r024-218-parser-chunking",
            path="scripts/replay_r024_218_document_parser_chunking.py",
            category=ScriptCategory.PARSER_REPLAY,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S03",
            contract=_contract(
                inputs=["data/article_catalog/article_catalog/index.json"],
                expected_outputs=[
                    "data/r024-218-document-corpus-v1/parser-chunking/summary.json",
                    "data/r024-218-document-corpus-v1/parser-chunking/events.jsonl",
                ],
                verification=["uv run pytest tests/test_r024_218_document_parser_chunking.py"],
                summary_fields=["completed", "metadata_only_skipped", "errors", "chunks"],
            ),
            notes="Primary parser replay extraction target because it includes PDF cache, source resolution, metadata-only skip, and M121-scale output.",
        ),
        _item(
            script_id="networkx-r024-10-probe",
            path="scripts/build_r024_networkx_probe.py",
            category=ScriptCategory.GRAPH_PROBE,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S05",
            contract=_contract(
                inputs=["data/r024-10-document-corpus-v1/parser-chunking/events.jsonl"],
                expected_outputs=["data/r024-10-document-corpus-v1/networkx-probe/summary.json"],
                verification=["uv run pytest tests/test_r024_networkx_probe.py"],
                summary_fields=["nodes", "edges", "citation_relations"],
            ),
            notes="Small graph probe variant; currently builds NetworkX graph directly in the script.",
        ),
        _item(
            script_id="networkx-r024-20-probe",
            path="scripts/build_r024_20_document_networkx_probe.py",
            category=ScriptCategory.GRAPH_PROBE,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S05",
            contract=_contract(
                inputs=["data/r024-20-document-corpus-v1/parser-chunking/events.jsonl"],
                expected_outputs=["data/r024-20-document-corpus-v1/networkx-probe/summary.json"],
                verification=["uv run pytest tests/test_r024_20_document_networkx_probe.py"],
                summary_fields=["nodes", "edges", "citation_relations"],
            ),
            notes="20-document graph probe variant; migrate to NetworkX adapter behind graph probe port.",
        ),
        _item(
            script_id="networkx-r024-53-probe",
            path="scripts/build_r024_53_document_networkx_probe.py",
            category=ScriptCategory.GRAPH_PROBE,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S05",
            contract=_contract(
                inputs=["data/r024-53-document-corpus-v1/parser-chunking/events.jsonl"],
                expected_outputs=[
                    "data/r024-53-document-corpus-v1/networkx-probe/summary.json",
                    "data/r024-53-document-corpus-v1/networkx-probe/memory-profile.json",
                ],
                verification=["uv run pytest tests/test_r024_53_document_networkx_probe.py"],
                summary_fields=["nodes", "edges", "citation_relations", "peak_memory_mb"],
            ),
            notes="53-document graph probe variant with memory profile output.",
        ),
        _item(
            script_id="networkx-r024-218-probe",
            path="scripts/build_r024_218_document_networkx_probe.py",
            category=ScriptCategory.GRAPH_PROBE,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S05",
            contract=_contract(
                inputs=["data/r024-218-document-corpus-v1/parser-chunking/events.jsonl"],
                expected_outputs=[
                    "data/r024-218-document-corpus-v1/networkx-probe/summary.json",
                    "data/r024-218-document-corpus-v1/networkx-probe/memory-profile.json",
                ],
                verification=["uv run pytest tests/test_r024_218_document_networkx_probe.py"],
                summary_fields=["nodes", "edges", "citation_relations", "peak_memory_mb"],
            ),
            notes="Primary graph probe extraction target because it is M121-scale and fully script-bound today.",
        ),
        _item(
            script_id="networkx-r024-entity-probe",
            path="scripts/build_r024_entity_networkx_probe.py",
            category=ScriptCategory.GRAPH_PROBE,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S05",
            contract=_contract(
                inputs=["data/r024-entity-scale-corpus-v1/entities-summary.json"],
                expected_outputs=[
                    "data/r024-entity-scale-corpus-v1/networkx-probe/summary.json",
                    "data/r024-entity-scale-corpus-v1/networkx-probe/memory-profile.json",
                ],
                verification=["uv run pytest tests/test_r024_entity_networkx_probe.py"],
                summary_fields=["nodes", "edges", "entity_nodes", "peak_memory_mb"],
            ),
            notes="Entity-scale probe is recurring but may remain follow-up if S05 scope narrows to document graph paths.",
        ),
        _item(
            script_id="extract-r024-quality-metrics",
            path="scripts/extract_r024_quality_metrics.py",
            category=ScriptCategory.QUALITY_METRICS,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S04",
            contract=_contract(
                inputs=["data/r024-10-document-corpus-v1/parser-chunking/events.jsonl"],
                expected_outputs=[
                    "data/r024-10-document-corpus-v1/quality-metrics.json",
                    "data/r024-10-document-corpus-v1/quality-comparison-5-vs-10.md",
                ],
                verification=["uv run pytest tests/test_r024_quality_metrics.py"],
                summary_fields=["article_count", "chunk_count", "comparison"],
            ),
            notes="First-class quality metrics category so report scripts are not lost between coverage and graph migration.",
        ),
        _item(
            script_id="extract-r024-20-quality-metrics",
            path="scripts/extract_r024_20_document_quality_metrics.py",
            category=ScriptCategory.QUALITY_METRICS,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S04",
            contract=_contract(
                inputs=["data/r024-20-document-corpus-v1/parser-chunking/events.jsonl"],
                expected_outputs=[
                    "data/r024-20-document-corpus-v1/quality-metrics.json",
                    "data/r024-20-document-corpus-v1/quality-comparison-10-vs-20.md",
                ],
                verification=["uv run pytest tests/test_r024_20_document_quality_metrics.py"],
                summary_fields=["article_count", "chunk_count", "comparison"],
            ),
            notes="20-document quality metrics report path for S04 coverage/reporting migration.",
        ),
        _item(
            script_id="extract-r024-53-quality-metrics",
            path="scripts/extract_r024_53_document_quality_metrics.py",
            category=ScriptCategory.QUALITY_METRICS,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S04",
            contract=_contract(
                inputs=["data/r024-53-document-corpus-v1/parser-chunking/events.jsonl"],
                expected_outputs=[
                    "data/r024-53-document-corpus-v1/quality-metrics.json",
                    "data/r024-53-document-corpus-v1/quality-comparison-20-vs-53.md",
                ],
                verification=["uv run pytest tests/test_r024_53_document_quality_metrics.py"],
                summary_fields=["article_count", "chunk_count", "comparison"],
            ),
            notes="53-document quality metrics report path for S04 coverage/reporting migration.",
        ),
        _item(
            script_id="extract-r024-entity-quality-metrics",
            path="scripts/extract_r024_entity_quality_metrics.py",
            category=ScriptCategory.QUALITY_METRICS,
            classification=ScriptClassification.PRODUCTION_CANDIDATE,
            migration_slice="S04",
            contract=_contract(
                inputs=["data/r024-entity-scale-corpus-v1/entities-summary.json"],
                expected_outputs=[
                    "data/r024-entity-scale-corpus-v1/quality-metrics.json",
                    "data/r024-entity-scale-corpus-v1/comparison-5-entities-vs-10-entities.md",
                ],
                verification=["uv run pytest tests/test_r024_entity_quality_metrics.py"],
                summary_fields=["entity_count", "relation_count", "comparison"],
            ),
            notes="Entity-scale quality metrics path is explicitly classified for S04 or follow-up planning.",
        ),
    ]
    return ScriptInventory(items=items)


def write_inventory(inventory: ScriptInventory, path: Path) -> list[ValidationIssue]:
    issues = validate_inventory(inventory)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(inventory.to_dict(), indent=2, sort_keys=True) + "\n")
    return issues
