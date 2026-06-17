from __future__ import annotations

import importlib
from pathlib import Path


EXPECTED_PACKAGES = (
    "research_graph",
    "research_graph.corpus",
    "research_graph.corpus.ingestion",
    "research_graph.corpus.parsing",
    "research_graph.corpus.sources",
    "research_graph.papers",
    "research_graph.papers.artifacts",
    "research_graph.papers.chunking",
    "research_graph.papers.indexing",
    "research_graph.papers.source_assets",
    "research_graph.graph",
    "research_graph.staging",
    "research_graph.identity",
    "research_graph.llm",
    "research_graph.evaluation",
    "research_graph.repair",
    "research_graph.workflows",
)


def test_research_graph_skeleton_packages_are_importable() -> None:
    for package in EXPECTED_PACKAGES:
        module = importlib.import_module(package)
        assert module.__name__ == package


def test_research_graph_skeleton_matches_target_design() -> None:
    target_map = Path("artifacts/package-rename-design/target-map.md").read_text(encoding="utf-8")

    assert "src/research_graph/" in target_map
    for context in ("corpus", "papers", "graph", "staging", "identity", "llm", "evaluation", "repair", "workflows"):
        assert f"{context}/" in target_map

    for package in EXPECTED_PACKAGES:
        expected_path = Path("src", *package.split("."))
        assert Path(expected_path, "__init__.py").exists()


def test_wave_00_manifest_is_skeleton_only() -> None:
    manifest_path = Path("archive/package-rename-waves/wave-00/manifest.md")
    manifest = manifest_path.read_text(encoding="utf-8")

    assert manifest_path.exists()
    assert "M088-umuqj7" in manifest
    assert "skeleton-only" in manifest
    assert "No implementation modules moved in wave 00." in manifest
    assert "No `arxiv_archive` files were archived by this wave." in manifest


def test_wave_00_manifest_remains_skeleton_only_history() -> None:
    manifest = Path("archive/package-rename-waves/wave-00/manifest.md").read_text(encoding="utf-8")

    assert "No implementation modules moved in wave 00." in manifest
    assert "Next wave" in manifest
    assert "Move already-canonical low-risk modules" in manifest


def test_wave_03_archives_paper_indexing_modules_without_runtime_shims() -> None:
    moves = {
        "article_links_dedup.py": "src/research_graph/papers/indexing/links_dedup.py",
        "article_page_index.py": "src/research_graph/papers/indexing/page_index.py",
        "article_retrieval_tables.py": "src/research_graph/papers/indexing/retrieval_tables.py",
    }
    manifest = Path("archive/package-rename-waves/wave-03/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-03/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest


def test_wave_04_archives_lower_level_indexing_package_without_runtime_shims() -> None:
    moves = {
        "navigation.py": "src/research_graph/papers/indexing/navigation.py",
        "page_index.py": "src/research_graph/papers/indexing/parsed_page_index.py",
    }
    manifest = Path("archive/package-rename-waves/wave-04/manifest.md").read_text(encoding="utf-8")

    old_runtime_py = sorted(Path("src/arxiv_archive/indexing").glob("*.py"))
    assert old_runtime_py == []
    assert Path("archive/package-rename-waves/wave-04/src/arxiv_archive/indexing/__init__.py").exists()
    assert "no runtime shim" in manifest
    assert "parsed_page_index.py" in manifest

    for old_name, new_path in moves.items():
        archived = Path("archive/package-rename-waves/wave-04/src/arxiv_archive/indexing") / old_name
        canonical = Path(new_path)

        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/indexing/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/indexing/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest


def test_wave_05_archives_top_level_page_index_bridge_without_runtime_shim() -> None:
    old_runtime = Path("src/arxiv_archive/page_index.py")
    archived = Path("archive/package-rename-waves/wave-05/src/arxiv_archive/page_index.py")
    manifest = Path("archive/package-rename-waves/wave-05/manifest.md").read_text(encoding="utf-8")

    assert not old_runtime.exists()
    assert archived.exists()
    assert "Formerly: src/arxiv_archive/page_index.py" in archived.read_text(encoding="utf-8")
    assert "`src/arxiv_archive/page_index.py`" in manifest
    assert "`src/research_graph/papers/indexing/__init__.py`" in manifest
    assert "no compatibility shim" in manifest


def test_wave_06_archives_source_assets_and_chunking_without_runtime_shims() -> None:
    moves = {
        "assets/registry.py": "src/research_graph/papers/source_assets/registry.py",
        "assets/provenance.py": "src/research_graph/papers/source_assets/provenance.py",
        "chunking/chunker.py": "src/research_graph/papers/chunking/chunker.py",
        "chunking/figure_units.py": "src/research_graph/papers/chunking/figure_units.py",
        "chunking/table_units.py": "src/research_graph/papers/chunking/table_units.py",
    }
    bridges = {
        "source_asset_manifest.py": "archive/package-rename-waves/wave-06/src/arxiv_archive/source_asset_manifest.py",
        "structure_aware_chunking.py": "archive/package-rename-waves/wave-06/src/arxiv_archive/structure_aware_chunking.py",
    }
    manifest = Path("archive/package-rename-waves/wave-06/manifest.md").read_text(encoding="utf-8")

    assert sorted(Path("src/arxiv_archive/assets").glob("*.py")) == []
    assert sorted(Path("src/arxiv_archive/chunking").glob("*.py")) == []
    assert not Path("src/arxiv_archive/source_asset_manifest.py").exists()
    assert not Path("src/arxiv_archive/structure_aware_chunking.py").exists()
    assert "no compatibility shims" in manifest
    assert "research_graph.papers.source_assets" in manifest
    assert "research_graph.papers.chunking" in manifest

    for old_relative, new_path in moves.items():
        archived = Path("archive/package-rename-waves/wave-06/src/arxiv_archive") / old_relative
        canonical = Path(new_path)

        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_relative}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_relative}`" in manifest
        assert f"`{new_path}`" in manifest

    for old_name, archive_path in bridges.items():
        archived = Path(archive_path)
        assert archived.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest


def test_wave_07_archives_corpus_ingestion_and_parsing_without_runtime_shims() -> None:
    moves = {
        "ingestion/fetchers.py": "src/research_graph/corpus/ingestion/fetchers.py",
        "ingestion/loader.py": "src/research_graph/corpus/ingestion/loader.py",
        "ingestion/logging.py": "src/research_graph/corpus/ingestion/logging.py",
        "parsing/normalization.py": "src/research_graph/corpus/parsing/normalization.py",
        "parsing/parser.py": "src/research_graph/corpus/parsing/parser.py",
        "parsing/structure.py": "src/research_graph/corpus/parsing/structure.py",
    }
    bridges = {
        "full_text.py": "archive/package-rename-waves/wave-07/src/arxiv_archive/full_text.py",
        "article_loader.py": "archive/package-rename-waves/wave-07/src/arxiv_archive/article_loader.py",
        "pdf_downloader.py": "archive/package-rename-waves/wave-07/src/arxiv_archive/pdf_downloader.py",
    }
    manifest = Path("archive/package-rename-waves/wave-07/manifest.md").read_text(encoding="utf-8")

    assert sorted(Path("src/arxiv_archive/ingestion").glob("*.py")) == []
    assert sorted(Path("src/arxiv_archive/parsing").glob("*.py")) == []
    assert not Path("src/arxiv_archive/full_text.py").exists()
    assert not Path("src/arxiv_archive/article_loader.py").exists()
    assert not Path("src/arxiv_archive/pdf_downloader.py").exists()
    assert Path("archive/package-rename-waves/wave-07/src/arxiv_archive/ingestion/__init__.py").exists()
    assert Path("archive/package-rename-waves/wave-07/src/arxiv_archive/parsing/__init__.py").exists()
    assert "no compatibility shims" in manifest
    assert "research_graph.corpus.ingestion" in manifest
    assert "research_graph.corpus.parsing" in manifest
    assert "md_converter.py` remains in place" in manifest

    for old_relative, new_path in moves.items():
        archived = Path("archive/package-rename-waves/wave-07/src/arxiv_archive") / old_relative
        canonical = Path(new_path)

        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_relative}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_relative}`" in manifest
        assert f"`{new_path}`" in manifest

    for old_name, archive_path in bridges.items():
        archived = Path(archive_path)
        assert archived.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest


def test_wave_08_archives_markdown_converter_without_runtime_shim() -> None:
    old_runtime = Path("src/arxiv_archive/md_converter.py")
    archived = Path("archive/package-rename-waves/wave-08/src/arxiv_archive/md_converter.py")
    canonical = Path("src/research_graph/corpus/sources/markdown_converter.py")
    manifest = Path("archive/package-rename-waves/wave-08/manifest.md").read_text(encoding="utf-8")

    assert not old_runtime.exists()
    assert archived.exists()
    assert canonical.exists()
    assert "Formerly: src/arxiv_archive/md_converter.py" in archived.read_text(encoding="utf-8")
    assert "Formerly: src/arxiv_archive/md_converter.py" in canonical.read_text(encoding="utf-8")
    assert "`src/arxiv_archive/md_converter.py`" in manifest
    assert "`src/research_graph/corpus/sources/markdown_converter.py`" in manifest
    assert "no compatibility shim" in manifest
    assert "must not perform live arxiv2md" in manifest


def test_wave_09_archives_thirty_paper_source_scan_without_runtime_shim() -> None:
    old_runtime = Path("src/arxiv_archive/thirty_paper_source_scan.py")
    archived = Path("archive/package-rename-waves/wave-09/src/arxiv_archive/thirty_paper_source_scan.py")
    canonical = Path("src/research_graph/corpus/sources/thirty_paper_source_scan.py")
    manifest = Path("archive/package-rename-waves/wave-09/manifest.md").read_text(encoding="utf-8")

    assert not old_runtime.exists()
    assert archived.exists()
    assert canonical.exists()
    assert "Formerly: src/arxiv_archive/thirty_paper_source_scan.py" in archived.read_text(encoding="utf-8")
    assert "Formerly: src/arxiv_archive/thirty_paper_source_scan.py" in canonical.read_text(encoding="utf-8")
    assert "`src/arxiv_archive/thirty_paper_source_scan.py`" in manifest
    assert "`src/research_graph/corpus/sources/thirty_paper_source_scan.py`" in manifest
    assert "no compatibility shim" in manifest
    assert "must not perform live arxiv2md" in manifest


def test_wave_10_archives_identity_and_staging_without_runtime_shims() -> None:
    moves = {
        "identity/__init__.py": "src/research_graph/identity/__init__.py",
        "identity/canonicalization.py": "src/research_graph/identity/canonicalization.py",
        "identity/dedup.py": "src/research_graph/identity/dedup.py",
        "staging/__init__.py": "src/research_graph/staging/__init__.py",
        "staging/graph_candidates.py": "src/research_graph/staging/graph_candidates.py",
        "staging/import_boundary.py": "src/research_graph/staging/import_boundary.py",
    }
    manifest = Path("archive/package-rename-waves/wave-10/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-10/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert importlib.import_module("research_graph.identity.canonicalization")
    assert importlib.import_module("research_graph.identity.dedup")
    assert importlib.import_module("research_graph.staging.graph_candidates")
    assert importlib.import_module("research_graph.staging.import_boundary")


def test_wave_11_archives_quality_package_without_runtime_shims() -> None:
    moves = {
        "quality/__init__.py": "src/research_graph/quality/__init__.py",
        "quality/baselines.py": "src/research_graph/quality/baselines.py",
        "quality/maintainability_report.py": "src/research_graph/quality/maintainability_report.py",
        "quality/riskratchet_adapter.py": "src/research_graph/quality/riskratchet_adapter.py",
        "quality/scopes.py": "src/research_graph/quality/scopes.py",
        "quality/thresholds.py": "src/research_graph/quality/thresholds.py",
    }
    manifest = Path("archive/package-rename-waves/wave-11/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-11/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert "No compatibility shim" in manifest
    assert importlib.import_module("research_graph.quality")
    assert importlib.import_module("research_graph.quality.maintainability_report")


def test_wave_13_archives_extraction_and_evaluation_without_runtime_shims() -> None:
    moves = {
        "dspy_extraction.py": "src/research_graph/evaluation/dspy_extraction.py",
        "extraction_benchmark.py": "src/research_graph/evaluation/extraction_benchmark.py",
        "scientific_extraction.py": "src/research_graph/evaluation/scientific_extraction.py",
        "evaluation.py": "src/research_graph/evaluation/metrics.py",
    }
    manifest = Path("archive/package-rename-waves/wave-13/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-13/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert "DSPy optimizer/provider" in manifest
    assert "scoring.py" in manifest
    assert importlib.import_module("research_graph.evaluation.dspy_extraction")
    assert importlib.import_module("research_graph.evaluation.extraction_benchmark")
    assert importlib.import_module("research_graph.evaluation.scientific_extraction")
    assert importlib.import_module("research_graph.evaluation.metrics")


def test_wave_12_archives_repair_cluster_without_runtime_shims() -> None:
    moves = {
        "bounded_chunk_repair.py": "src/research_graph/repair/bounded_chunk_repair.py",
        "candidate_locators.py": "src/research_graph/repair/candidate_locators.py",
        "chunking_benchmark.py": "src/research_graph/repair/chunking_benchmark.py",
    }
    manifest = Path("archive/package-rename-waves/wave-12/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-12/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert "chunk_repair_contract" in manifest
    assert "chunk_import_contract" in manifest
    assert "chunk_baseline_measurement" in manifest
    assert "S09" in manifest or "S12" in manifest
    assert importlib.import_module("research_graph.repair.bounded_chunk_repair")
    assert importlib.import_module("research_graph.repair.candidate_locators")
    assert importlib.import_module("research_graph.repair.chunking_benchmark")


def test_wave_14_archives_retrieval_cluster_without_runtime_shims() -> None:
    moves = {
        "embedder.py": "src/research_graph/retrieval/embedder.py",
        "hybrid_retrieval.py": "src/research_graph/retrieval/hybrid.py",
        "keyword_extractor.py": "src/research_graph/retrieval/keyword_extractor.py",
        "summarizer.py": "src/research_graph/retrieval/summarizer.py",
    }
    manifest = Path("archive/package-rename-waves/wave-14/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-14/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert "MiniMaxSummarizer" in manifest
    assert importlib.import_module("research_graph.retrieval.embedder")
    assert importlib.import_module("research_graph.retrieval.hybrid")
    assert importlib.import_module("research_graph.retrieval.keyword_extractor")


def test_wave_15_archives_external_clients_without_runtime_shims() -> None:
    moves = {
        "arxiv_client.py": "src/research_graph/corpus/sources/arxiv_client.py",
        "semantic_scholar.py": "src/research_graph/corpus/sources/semantic_scholar.py",
        "ladybug_client.py": "src/research_graph/graph/ladybug_client.py",
        "telegram_sender.py": "src/research_graph/ops/notifications/telegram_sender.py",
    }
    manifest = Path("archive/package-rename-waves/wave-15/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-15/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert "no permanent" not in manifest.lower() and "compatibility shim" not in manifest.lower()
    assert importlib.import_module("research_graph.corpus.sources.arxiv_client")
    assert importlib.import_module("research_graph.corpus.sources.semantic_scholar")
    assert importlib.import_module("research_graph.graph.ladybug_client")
    assert importlib.import_module("research_graph.ops.notifications.telegram_sender")


def test_wave_16_archives_validation_batch_workflow_without_runtime_shims() -> None:
    moves = {
        "validation_batch_provenance.py": "src/research_graph/workflows/validation/batch_provenance.py",
        "validation_batch_state.py": "src/research_graph/workflows/validation/batch_state.py",
        "validation_batch_workflow.py": "src/research_graph/workflows/validation/batch_workflow.py",
        "validation_logging.py": "src/research_graph/workflows/validation/logging.py",
    }
    manifest = Path("archive/package-rename-waves/wave-16/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-16/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert "local-only" in manifest
    assert importlib.import_module("research_graph.workflows.validation.batch_provenance")
    assert importlib.import_module("research_graph.workflows.validation.batch_state")
    assert importlib.import_module("research_graph.workflows.validation.batch_workflow")
    assert importlib.import_module("research_graph.workflows.validation.logging")


def test_wave_17_archives_universal_kb_without_runtime_shims() -> None:
    moves = {
        "universal_kb_contracts.py": "src/research_graph/workflows/universal_kb/contracts.py",
        "universal_kb_queue.py": "src/research_graph/workflows/universal_kb/queue.py",
        "universal_kb_rehearsal.py": "src/research_graph/workflows/universal_kb/rehearsal.py",
        "universal_kb_review_assistance.py": "src/research_graph/workflows/universal_kb/review_assistance.py",
        "universal_kb_sidecar_boundary.py": "src/research_graph/workflows/universal_kb/sidecar_boundary.py",
        "universal_kb_smoke.py": "src/research_graph/workflows/universal_kb/smoke.py",
        "universal_kb_substrate_rehearsal.py": "src/research_graph/workflows/universal_kb/substrate_rehearsal.py",
    }
    manifest = Path("archive/package-rename-waves/wave-17/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-17/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert "fact-promotion" in manifest
    assert importlib.import_module("research_graph.workflows.universal_kb.contracts")
    assert importlib.import_module("research_graph.workflows.universal_kb.queue")
    assert importlib.import_module("research_graph.workflows.universal_kb.rehearsal")


def test_wave_18_archives_graph_readiness_without_runtime_shims() -> None:
    moves = {
        "graph_readiness.py": "src/research_graph/graph/readiness/core.py",
        "graph_readiness_export.py": "src/research_graph/graph/readiness/export.py",
        "graph_readiness_extraction_gate.py": "src/research_graph/graph/readiness/extraction_gate.py",
        "graph_readiness_manifest.py": "src/research_graph/graph/readiness/manifest.py",
        "graph_readiness_persistence.py": "src/research_graph/graph/readiness/persistence.py",
        "graph_readiness_retrieval_validation.py": "src/research_graph/graph/readiness/retrieval_validation.py",
        "graph_readiness_review.py": "src/research_graph/graph/readiness/review.py",
    }
    manifest = Path("archive/package-rename-waves/wave-18/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-18/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert "review" in manifest
    assert importlib.import_module("research_graph.graph.readiness.core")
    assert importlib.import_module("research_graph.graph.readiness.review")


def test_wave_19_archives_rlm_without_runtime_shims() -> None:
    moves = {
        "rlm_graph_traversal.py": "src/research_graph/workflows/rlm/graph_traversal.py",
        "rlm_workflow.py": "src/research_graph/workflows/rlm/workflow.py",
    }
    manifest = Path("archive/package-rename-waves/wave-19/manifest.md").read_text(encoding="utf-8")

    for old_name, new_path in moves.items():
        old_runtime = Path("src/arxiv_archive") / old_name
        archived = Path("archive/package-rename-waves/wave-19/src/arxiv_archive") / old_name
        canonical = Path(new_path)

        assert not old_runtime.exists()
        assert archived.exists()
        assert canonical.exists()
        assert f"Formerly: src/arxiv_archive/{old_name}" in archived.read_text(encoding="utf-8")
        assert f"Formerly: src/arxiv_archive/{old_name}" in canonical.read_text(encoding="utf-8")
        assert f"`src/arxiv_archive/{old_name}`" in manifest
        assert f"`{new_path}`" in manifest

    assert importlib.import_module("research_graph.workflows.rlm.graph_traversal")
    assert importlib.import_module("research_graph.workflows.rlm.workflow")

