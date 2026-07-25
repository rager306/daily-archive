"""Contract tests for the M103 S02 pipeline framework (ADR-033, D085).

These tests pin the Level 1-4 contracts so the M104 hexagonal refactor (Ports/
Adapters) and any Phase 4 scheduler activation cannot silently regress the
framework's seams. They are deterministic and require no LLM, no network, no
graph writes (fail-closed throughout).
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import replace

import pytest

from research_graph.application import (
    Pipeline,
    PipelineContext,
    PipelineStage,
    ResourceProfile,
    StageManifest,
)
from research_graph.application.orchestrator import (
    AdmissionError,
    DispatchProtocol,
    PipelineOrchestrator,
    QueueDispatch,
    SyncDispatch,
    can_dispatch,
)
from research_graph.application.primitives import (
    BinaryRelationDetector,
    CoreEntityExtractor,
    EvidenceLinker,
    RelationTypeClassifier,
    StatisticalContext,
    StatisticalPreProcessor,
)
from research_graph.application.profiles import build_paper_pipeline
from research_graph.application.profiles.paper import PAPER_STAGE_ORDER
from research_graph.domain.relation_types import ALL_TYPED_RELATIONS
from research_graph.domain.schema import DEFAULT_SAFETY_FLAGS, ExtractionPatch

_TEXT_PARTS = [
    "transformers enable attention mechanisms for reasoning",
    "attention improves transformers reasoning quality",
    "transformers need attention to reason over long context",
]


def _seed_ctx(source_id: str = "arxiv:2605.18747") -> PipelineContext:
    return replace(PipelineContext(source_id=source_id), stage_outputs={"text_parts": _TEXT_PARTS})


def _fake_keywords(text_parts: Sequence[str], top_k: int) -> list[str]:
    """Deterministic test keyword extractor (stands in for the infra YAKE one)."""
    from collections import Counter

    words = [w for part in text_parts for w in part.lower().split() if len(w) > 2]
    return [w for w, _ in Counter(words).most_common(top_k)]


# ── Level 1: types.py ────────────────────────────────────────────────────────


class TestResourceProfile:
    def test_defaults_match_adr_027(self) -> None:
        rp = ResourceProfile()
        assert rp.llm_required is False
        assert rp.llm_provider is None
        assert rp.estimated_tokens == 0
        assert rp.cpu_required is False
        assert rp.cpu_intensity == "light"
        assert rp.io_required is False
        assert rp.io_type is None

    def test_llm_lane_fields(self) -> None:
        rp = ResourceProfile(llm_required=True, llm_provider="minimax", estimated_tokens=2048)
        assert rp.llm_required and rp.llm_provider == "minimax" and rp.estimated_tokens == 2048


class TestPipelineContext:
    def test_placeholders_none_in_foundation(self) -> None:
        ctx = PipelineContext(source_id="s")
        assert ctx.statistical_context is None
        assert ctx.resource_state is None

    def test_with_output_is_frozen_safe(self) -> None:
        ctx = PipelineContext(source_id="s")
        ctx2 = ctx.with_output("a", {"x": 1})
        assert ctx2.stage_outputs["a"] == {"x": 1}
        assert ctx.stage_outputs == {}  # original unchanged


class TestPipeline:
    def test_run_threads_stages_in_order(self) -> None:
        @__import__("dataclasses").dataclass(frozen=True)
        class _S:
            stage_name: str
            resource_profile: ResourceProfile = ResourceProfile()

            def run(self, context):
                return context.with_output(self.stage_name, "ok")

        # pyrefly: ignore [bad-argument-count]
        pipe = Pipeline(stages=(_S("a"), _S("b")), source_id="s")
        ctx = pipe.run()
        assert ctx.stage_outputs == {"a": "ok", "b": "ok"}

    def test_empty_pipeline_raises(self) -> None:
        with pytest.raises(ValueError):
            Pipeline().run()

    def test_manifest_is_queue_submittable(self) -> None:
        @__import__("dataclasses").dataclass(frozen=True)
        class _S:
            stage_name: str = "x"
            resource_profile: ResourceProfile = ResourceProfile()

            def run(self, context):
                return context

        manifest = Pipeline(stages=(_S(),), source_id="s").manifest()
        assert len(manifest) == 1
        assert isinstance(manifest[0], StageManifest)
        assert manifest[0].stage_name == "x"


# ── Level 2: primitives.py ───────────────────────────────────────────────────


class TestPrimitivesLanes:
    def test_all_five_stages_satisfy_protocol(self) -> None:
        for stage in (
            StatisticalPreProcessor(keyword_extractor=_fake_keywords),
            CoreEntityExtractor(),
            BinaryRelationDetector(),
            RelationTypeClassifier(),
            EvidenceLinker(),
        ):
            # pyrefly: ignore [unsafe-overlap]
            assert isinstance(stage, PipelineStage)

    def test_llm_stages_require_llm(self) -> None:
        assert CoreEntityExtractor().resource_profile.llm_required
        assert RelationTypeClassifier().resource_profile.llm_required

    def test_cpu_stages_do_not_require_llm(self) -> None:
        assert not StatisticalPreProcessor().resource_profile.llm_required
        assert not BinaryRelationDetector().resource_profile.llm_required
        assert not EvidenceLinker().resource_profile.llm_required


class TestStatisticalPreProcessor:
    def test_stub_without_extractor_emits_empty(self) -> None:
        ctx = StatisticalPreProcessor().run(_seed_ctx())
        assert isinstance(ctx.statistical_context, StatisticalContext)
        # No injected extractor -> stub emits empty keywords (application layer
        # never imports the infrastructure KeywordExtractor).
        assert len(ctx.statistical_context.keywords) == 0

    def test_yake_keywords_deterministic(self) -> None:
        ctx = StatisticalPreProcessor(keyword_extractor=_fake_keywords).run(_seed_ctx())
        assert isinstance(ctx.statistical_context, StatisticalContext)
        assert len(ctx.statistical_context.keywords) > 0

    def test_co_occurrence(self) -> None:
        ctx = StatisticalPreProcessor(keyword_extractor=_fake_keywords, co_occurrence_min=2).run(
            _seed_ctx()
        )
        # pyrefly: ignore [missing-attribute]
        assert len(ctx.statistical_context.co_occurrence) > 0  # ty:ignore[unresolved-attribute]


class TestStubbedLLMStages:
    def test_core_extractor_stubbed_emits_empty_fail_closed(self) -> None:
        ctx = CoreEntityExtractor().run(
            StatisticalPreProcessor(keyword_extractor=_fake_keywords).run(_seed_ctx())
        )
        patch = ctx.stage_outputs["core_entity_extractor"]
        assert isinstance(patch, ExtractionPatch)
        assert patch.entities == []
        assert patch.safety_flags["import_eligible"] is False

    def test_classifier_stubbed_never_invents_types(self) -> None:
        ctx = BinaryRelationDetector().run(
            StatisticalPreProcessor(keyword_extractor=_fake_keywords, co_occurrence_min=2).run(
                _seed_ctx()
            )
        )
        ctx = RelationTypeClassifier().run(ctx)
        patch = ctx.stage_outputs["relation_type_classifier"]
        assert isinstance(patch, ExtractionPatch)
        assert patch.relations == []  # stubbed: never invents typed relations


class TestRelationTypeClassifierConstraint:
    def test_drops_non_typed_relation_types(self) -> None:
        def mock_client(prompt, snapshot):
            return {
                "relations": [
                    {
                        "relation_type": "BUILDS_ON",
                        "from_name": "a",
                        "to_name": "b",
                        "confidence": 0.9,
                    },
                    {
                        "relation_type": "BOGUS_TYPE",
                        "from_name": "x",
                        "to_name": "y",
                        "confidence": 0.5,
                    },
                    {
                        "relation_type": "CAUSES",
                        "from_name": "c",
                        "to_name": "d",
                        "confidence": 0.7,
                    },
                ]
            }

        ctx = BinaryRelationDetector().run(
            StatisticalPreProcessor(co_occurrence_min=2).run(
                replace(
                    PipelineContext(source_id="s"),
                    stage_outputs={"text_parts": ["a b", "a b", "a b"]},
                )
            )
        )
        ctx = RelationTypeClassifier(llm_client=mock_client).run(ctx)
        types = [r.relation_type for r in ctx.stage_outputs["relation_type_classifier"].relations]
        assert "BOGUS_TYPE" not in types
        assert all(t in ALL_TYPED_RELATIONS for t in types)
        assert set(types) == {"BUILDS_ON", "CAUSES"}


class TestEvidenceLinker:
    def test_attaches_evidence_path(self) -> None:
        e = _make_entity("e1", "arxiv:2605.18747")
        ctx = replace(
            _seed_ctx(),
            stage_outputs={
                "text_parts": ["a"],
                "evidence_anchor": {
                    "page_index_node_id": "n3",
                    "semantic_chunk_id": "c7",
                    "node_path": ["sec1", "sec1.2"],
                },
                "core_entity_extractor": ExtractionPatch(
                    source_id="arxiv:2605.18747",
                    claims=[],
                    entities=[e],
                    relations=[],
                    safety_flags=dict(DEFAULT_SAFETY_FLAGS),
                ),
            },
        )
        result = EvidenceLinker().run(ctx).stage_outputs["evidence_linker"]
        assert isinstance(result, ExtractionPatch)
        assert result.entities[0].evidence_path is not None
        assert result.entities[0].evidence_path.semantic_chunk_id == "c7"


def _make_entity(eid: str, source_id: str):
    from research_graph.domain.schema import TypedEntity

    return TypedEntity(
        entity_id=eid,
        source_id=source_id,
        entity_type="method",
        canonical_name="attention",
        confidence=0.9,
        evidence_path=None,
        extractor_version="core.v1",
        safety_flags=dict(DEFAULT_SAFETY_FLAGS),
    )


# ── Level 3: paper profile ───────────────────────────────────────────────────


class TestPaperProfile:
    def test_stage_order_matches_core_then_modes(self) -> None:
        pipe = build_paper_pipeline(source_id="s")
        assert tuple(s.stage_name for s in pipe.stages) == PAPER_STAGE_ORDER

    def test_statistical_pre_processor_runs_before_llm(self) -> None:
        order = PAPER_STAGE_ORDER
        assert order.index("statistical_pre_processor") < order.index("core_entity_extractor")


# ── Level 4: orchestrator + dispatch seam ────────────────────────────────────


class TestOrchestrator:
    def test_sync_run_processes_all_stages(self) -> None:
        orch = PipelineOrchestrator(pipeline=build_paper_pipeline(source_id="s"))
        result = orch.run(_seed_ctx("s"))
        for stage in PAPER_STAGE_ORDER:
            assert stage in result.stage_outputs

    def test_admission_blocks_llm_stage_fail_closed(self) -> None:
        orch = PipelineOrchestrator(
            pipeline=build_paper_pipeline(source_id="s"),
            dispatch=SyncDispatch(llm_lane_check=lambda p, c: False),
        )
        with pytest.raises(AdmissionError):
            orch.run(_seed_ctx("s"))


class TestDispatchSeam:
    def test_sync_and_queue_both_satisfy_protocol(self) -> None:
        assert isinstance(SyncDispatch(), DispatchProtocol)

        class _FakeQueue:
            def enqueue(self, **kw):
                return {"job_id": kw["job_id"]}

            def claim(self, *, worker_id, lease_seconds):
                return {"job_id": "j1", "stage": "x"}

            def complete(self, job_id, *, worker_id, output_paths):
                return {}

            def fail_retryable(
                self, job_id, *, worker_id, error_code, redacted_message, retry_after
            ):
                return {}

        assert isinstance(QueueDispatch(queue=_FakeQueue()), DispatchProtocol)

    def test_can_dispatch_injectable_llm_check(self) -> None:
        from research_graph.application.primitives import (
            CoreEntityExtractor,
            StatisticalPreProcessor,
        )

        cpu = StatisticalPreProcessor(keyword_extractor=_fake_keywords)
        llm = CoreEntityExtractor()
        # pyrefly: ignore [bad-argument-type]
        assert can_dispatch(cpu, PipelineContext(source_id="s")) is True
        assert (
            # pyrefly: ignore [bad-argument-type]
            can_dispatch(llm, PipelineContext(source_id="s"), llm_lane_check=lambda p, c: False)
            is False
        )
        assert (
            # pyrefly: ignore [bad-argument-type]
            can_dispatch(llm, PipelineContext(source_id="s"), llm_lane_check=lambda p, c: True)
            is True
        )
