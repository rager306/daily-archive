"""M052 S02 deterministic RLM workflow + graph traversal e2e audit."""

from __future__ import annotations

import argparse
import json
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

import ladybug

import research_graph.infrastructure.graph.ladybug_client as ladybug_client
from research_graph.domain.semantic_chunks import EvidencePath
from research_graph.infrastructure.corpus.ingestion import FullTextSource, ingest_full_text
from research_graph.infrastructure.evaluation.evaluation_metrics import (
    calculate_evidence_path_hit_rate,
    calculate_retrieval_recall,
)
from research_graph.infrastructure.evaluation.scientific_extraction import (
    Claim,
    ExtractionPatch,
    ScientificEntity,
    ScientificRelation,
)
from research_graph.infrastructure.graph.ladybug_client import evidence_path_id
from research_graph.infrastructure.papers.indexing import PageIndexDocument, build_page_index
from research_graph.infrastructure.papers.semantic_chunks import (
    build_evidence_path,
    build_semantic_chunks,
)
from research_graph.infrastructure.retrieval.hybrid import InMemoryVectorCandidateIndex
from research_graph.workflows.rlm.graph_traversal import (
    ComparisonResult,
    RLMGraphTraversalConfig,
    RLMGraphTraversalQuestion,
    compare_rlm_graph_traversal,
)
from research_graph.workflows.rlm.workflow import run_document_workflow

REPO_ROOT = Path(__file__).resolve().parents[1]
ARTICLE_STRUCTURE_PATH = (
    REPO_ROOT / "tests" / "fixtures" / "article_artifacts" / "basic_article_structure.json"
)
FULL_TEXT_FIXTURE_PATH = REPO_ROOT / "tests" / "fixtures" / "full_text" / "structured_paper.md"
DEFAULT_OUTPUT_DIR = REPO_ROOT / "artifacts" / "m052-rlm-e2e"
SCHEMA_VERSION = "scientific_extraction.v1"
EXTRACTOR_VERSION = "m052-s02-e2e-fixture.v1"
SAFETY_KEYS = (
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
)


@dataclass(frozen=True)
class E2EGraphFixture:
    """Ephemeral in-memory graph fixture for traversal comparison."""

    conn: ladybug.Connection
    document: PageIndexDocument
    expected_semantic_chunk_ids: frozenset[str]
    expected_evidence_path_ids: frozenset[str]
    seed_semantic_chunk_ids: tuple[str, ...]
    seed_evidence_path_ids: tuple[str, ...]
    vector_index: InMemoryVectorCandidateIndex


def load_article_structure() -> dict[str, Any]:
    """Load the redacted article structure fixture used by the workflow harness."""
    return json.loads(ARTICLE_STRUCTURE_PATH.read_text(encoding="utf-8"))


def build_document() -> PageIndexDocument:
    """Build a deterministic PageIndex document from local markdown."""
    ingestion = ingest_full_text(
        FullTextSource(
            paper_id="2605.12345",
            source_type="markdown",
            source_path=FULL_TEXT_FIXTURE_PATH,
        )
    )
    return build_page_index(ingestion)


def build_fixture_patch(evidence: EvidencePath) -> ExtractionPatch:
    """Build a minimal validated scientific KG patch for the ephemeral fixture."""
    claim = Claim(
        claim_id="claim:2605.12345:method:chunk-0001:local-markdown-pageindex",
        source_id="2605.12345",
        text="Local markdown is enough to build a deterministic PageIndex.",
        claim_type="method",
        confidence=0.91,
        evidence_path=evidence,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "m052_s02_e2e_fixture"},
    )
    entity = ScientificEntity(
        entity_id="entity:2605.12345:pageindex",
        source_id="2605.12345",
        canonical_name="PageIndex",
        entity_type="method",
        confidence=0.88,
        evidence_path=evidence,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "m052_s02_e2e_fixture"},
    )
    relation = ScientificRelation(
        relation_id="relation:2605.12345:claim-local-markdown-pageindex:entity-pageindex:SUPPORTS",
        source_id="2605.12345",
        relation_type="SUPPORTS",
        from_entity_id=claim.claim_id,
        to_entity_id=entity.entity_id,
        confidence=0.84,
        evidence_path=evidence,
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "m052_s02_e2e_fixture"},
    )
    return ExtractionPatch(
        source_id="2605.12345",
        claims=[claim],
        entities=[entity],
        relations=[relation],
        schema_version=SCHEMA_VERSION,
        extractor_version=EXTRACTOR_VERSION,
        validation_warnings=[],
        provenance={"source": "m052_s02_e2e_fixture"},
    )


def _evidence_for_chunk(evidence_paths: list[EvidencePath], semantic_chunk_id: str) -> EvidencePath:
    for evidence in evidence_paths:
        if evidence.semantic_chunk_id == semantic_chunk_id:
            return evidence
    raise LookupError(f"missing_evidence_for_chunk:{semantic_chunk_id}")


def build_graph_fixture() -> E2EGraphFixture:
    """Create an ephemeral read-side fixture for S10 traversal comparison."""
    db = ladybug.Database(":memory:")
    conn = ladybug.Connection(db)
    ladybug_client.init_scientific_kg_schema(conn)

    document = build_document()
    chunks = build_semantic_chunks(document)
    evidence_paths = [build_evidence_path(document, chunk) for chunk in chunks]
    method_chunk = next(
        chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:method"
    )
    conclusion_chunk = next(
        chunk for chunk in chunks if chunk.page_index_node_id == "2605.12345:conclusion"
    )
    method_evidence = _evidence_for_chunk(evidence_paths, method_chunk.id)
    conclusion_evidence = _evidence_for_chunk(evidence_paths, conclusion_chunk.id)

    ladybug_client.upsert_scientific_kg(
        conn, document, chunks, evidence_paths, build_fixture_patch(method_evidence)
    )

    vector_index = InMemoryVectorCandidateIndex(
        {
            method_chunk.id: (1.0, 0.0, 0.0),
            conclusion_chunk.id: (0.92, 0.08, 0.0),
        }
    )
    return E2EGraphFixture(
        conn=conn,
        document=document,
        expected_semantic_chunk_ids=frozenset({method_chunk.id, conclusion_chunk.id}),
        expected_evidence_path_ids=frozenset(
            {evidence_path_id(method_evidence), evidence_path_id(conclusion_evidence)}
        ),
        seed_semantic_chunk_ids=(method_chunk.id,),
        seed_evidence_path_ids=(evidence_path_id(method_evidence),),
        vector_index=vector_index,
    )


def extract_helper_candidate_set(trajectory_steps: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Extract deterministic candidate records from helper_invoke trajectory steps."""
    candidates: list[dict[str, Any]] = []
    for step in trajectory_steps:
        if step.get("step_type") != "helper_invoke":
            continue
        diagnostics = step.get("diagnostics") if isinstance(step.get("diagnostics"), dict) else {}
        candidates.append(
            {
                "work_id": step.get("work_id"),
                "binding_id": diagnostics.get("binding_id"),  # ty:ignore[unresolved-attribute]
                "model_id": diagnostics.get("model_id"),  # ty:ignore[unresolved-attribute]
                "section_id": diagnostics.get("section_id"),  # ty:ignore[unresolved-attribute]
            }
        )
    return candidates


def _metrics_dict(comparison: ComparisonResult, fixture: E2EGraphFixture) -> dict[str, Any]:
    recall = calculate_retrieval_recall(
        comparison.rlm_traversal.candidates,
        fixture.expected_semantic_chunk_ids,
    )
    hit_rate = calculate_evidence_path_hit_rate(
        comparison.rlm_traversal.candidates,
        fixture.expected_evidence_path_ids,
    )
    return {
        "retrieval_recall": asdict(recall),
        "evidence_path_hit_rate": asdict(hit_rate),
    }


def _comparison_dict(comparison: ComparisonResult) -> dict[str, Any]:
    payload = asdict(comparison)
    payload["baseline_labels"] = [baseline.label for baseline in comparison.baselines]  # pyrefly: ignore[bad-assignment]
    payload["baseline_count"] = len(comparison.baselines)  # pyrefly: ignore[bad-assignment]
    return payload


def _audit_markdown(audit: dict[str, Any]) -> str:
    metrics = audit["metrics"]
    comparison = audit["comparison"]
    safety = audit["safety_defaults"]
    return "\n".join(
        [
            "# M052 S02 RLM e2e audit",
            "",
            f"- Trajectory steps: {audit['trajectory']['step_count']}",
            f"- Helper candidates: {len(audit['helper_candidate_set'])}",
            f"- Comparison question: {comparison['question_id']}",
            f"- RLM stop reason: {comparison['rlm_traversal']['stop_reason']}",
            f"- RLM retrieval recall: {metrics['retrieval_recall']['recall']}",
            f"- RLM evidence path hit rate: {metrics['evidence_path_hit_rate']['hit_rate']}",
            f"- Safety defaults all false: {safety['all_5_safety_defaults_false']}",
            "- Persistent graph writes: disabled; the traversal fixture is in-memory only.",
            "- Import authority: import is not authorized.",
            "- Network endpoint: 127.0.0.1 disabled for this deterministic audit.",
            "",
        ]
    )


def run_e2e(output_dir: Path = DEFAULT_OUTPUT_DIR) -> dict[str, Any]:
    """Run the deterministic S09 + S10 + S07 audit and write JSON/Markdown artifacts."""
    structure = load_article_structure()
    workflow = run_document_workflow(
        structure,
        page_index={"fixture": "basic_article_structure"},
        chunks=[],
        evidence_paths=[],
        run_id="m052-s02-e2e",
    )
    trajectory = workflow.trajectory.to_sanitized_dict()
    helper_candidate_set = extract_helper_candidate_set(trajectory["steps"])

    graph_fixture = build_graph_fixture()
    question = RLMGraphTraversalQuestion(
        name="m052-s02-e2e-pageindex",
        query="Which deterministic local evidence supports the PageIndex method?",
        query_vector=(1.0, 0.0, 0.0),
        seed_semantic_chunk_ids=graph_fixture.seed_semantic_chunk_ids,
        seed_evidence_path_ids=graph_fixture.seed_evidence_path_ids,
        expected_semantic_chunk_ids=graph_fixture.expected_semantic_chunk_ids,
        expected_evidence_path_ids=graph_fixture.expected_evidence_path_ids,
    )
    comparison = compare_rlm_graph_traversal(
        graph_fixture.conn,
        question,
        vector_index=graph_fixture.vector_index,
        config=RLMGraphTraversalConfig(max_steps=4, max_neighbors_per_step=3, top_k=4),
    )
    safety_defaults = dict(workflow.safety_audit["aggregate_safety_defaults"])
    audit = {
        "schema_version": "m052-s02-rlm-e2e-audit.v1",
        "fixture": {
            "article_structure_path": str(ARTICLE_STRUCTURE_PATH.relative_to(REPO_ROOT)),
            "full_text_fixture_path": str(FULL_TEXT_FIXTURE_PATH.relative_to(REPO_ROOT)),
            "graph_fixture": "ladybug://127.0.0.1/in-memory-disabled-network",
        },
        "trajectory": {
            "run_id": trajectory["run_id"],
            "step_count": len(trajectory["steps"]),
            "step_types": [step["step_type"] for step in trajectory["steps"]],
            "work_ids": trajectory["work_ids"],
        },
        "helper_candidate_set": helper_candidate_set,
        "comparison": _comparison_dict(comparison),
        "metrics": _metrics_dict(comparison, graph_fixture),
        "safety_defaults": {
            "keys": list(SAFETY_KEYS),
            "values": safety_defaults,
            "all_5_safety_defaults_false": set(safety_defaults) == set(SAFETY_KEYS)
            and all(value is False for value in safety_defaults.values()),
            "persistent_graph_writes": False,
            "network_endpoint": "127.0.0.1 disabled",
        },
    }

    output_dir.mkdir(parents=True, exist_ok=True)
    audit_json_path = output_dir / "audit.json"
    audit_md_path = output_dir / "audit.md"
    audit_json_path.write_text(
        json.dumps(audit, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    audit_md_path.write_text(_audit_markdown(audit), encoding="utf-8")
    return audit


def main() -> None:
    parser = argparse.ArgumentParser(description="Run M052 S02 deterministic RLM e2e audit.")
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    args = parser.parse_args()
    run_e2e(args.output_dir)


if __name__ == "__main__":
    main()
