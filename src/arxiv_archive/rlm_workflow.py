"""Read-only RLM document navigation workflow contracts.

This module provides the S09 fixture-level RLM harness boundary.  It accepts
already-built PageIndex/SemanticChunk/EvidencePath objects, validates navigation
before any downstream phase, and returns deterministic ID/count-only trajectory
records that are safe to repr or log.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Any, Literal

from arxiv_archive.dspy_extraction import (
    BaselineDspyExtractionModule,
    DspyExtractionInput,
    DspyExtractionOutput,
)
from arxiv_archive.evidence import EvidencePath, SemanticChunk, validate_evidence_path
from arxiv_archive.ladybug_client import evidence_path_id
from arxiv_archive.page_index import PageIndexDocument
from arxiv_archive.scientific_extraction import ExtractionPatch

WorkflowStatus = Literal["ok", "blocked", "warning"]
WorkflowPhase = Literal[
    "validate_navigation",
    "walk_next",
    "children",
    "path",
    "chunks",
    "evidence",
    "draft",
]


@dataclass(frozen=True)
class RLMWorkflowDiagnostic:
    """Text-safe diagnostic emitted by the read-only workflow harness."""

    phase: WorkflowPhase
    status: WorkflowStatus
    message: str
    node_id: str | None = None
    semantic_chunk_id: str | None = None
    evidence_path_id: str | None = None


@dataclass(frozen=True)
class RLMWorkflowStep:
    """One deterministic trajectory step containing only IDs, counts, and flags."""

    phase: WorkflowPhase
    status: WorkflowStatus
    node_id: str | None = None
    child_node_ids: tuple[str, ...] = ()
    path_node_ids: tuple[str, ...] = ()
    semantic_chunk_ids: tuple[str, ...] = ()
    evidence_path_ids: tuple[str, ...] = ()
    counts: tuple[tuple[str, int], ...] = ()
    diagnostics: tuple[RLMWorkflowDiagnostic, ...] = ()
    boundary_valid: bool | None = None
    schema_valid: bool | None = None
    groundedness_valid: bool | None = None
    optimizer_enabled: bool | None = None


@dataclass(frozen=True)
class RLMNavigationContext:
    """ID-only evidence context assembled for the extraction boundary."""

    paper_id: str
    root_node_id: str
    node_ids: tuple[str, ...]
    semantic_chunk_ids: tuple[str, ...]
    evidence_path_ids: tuple[str, ...]
    counts: tuple[tuple[str, int], ...]
    route_status: str = "navigation_validated"


@dataclass(frozen=True)
class RLMWorkflowResult:
    """Typed result from the read-only RLM document workflow harness."""

    status: WorkflowStatus
    navigation_valid: bool
    boundary_valid: bool | None
    context: RLMNavigationContext | None
    trajectory: tuple[RLMWorkflowStep, ...]
    diagnostics: tuple[RLMWorkflowDiagnostic, ...]
    draft: ExtractionPatch | None = field(default=None, repr=False)
    boundary_output: DspyExtractionOutput | None = field(default=None, repr=False)


def run_document_workflow(
    document: PageIndexDocument,
    *,
    chunks: list[SemanticChunk],
    evidence_paths: list[EvidencePath],
    extractor: BaselineDspyExtractionModule | None = None,
    optimizer_config: Mapping[str, Any] | None = None,
) -> RLMWorkflowResult:
    """Run a deterministic, read-only RLM workflow over fixture objects.

    The harness validates PageIndex navigation and local evidence references,
    then, when an S08 baseline extraction module is supplied, calls
    ``BaselineDspyExtractionModule.forward`` exactly once.  It does not build
    chunks, call LLMs, perform I/O, retry, or mutate inputs.  Expected invalid
    references are surfaced as typed diagnostics and warning trajectory steps.

    ``optimizer_config`` is accepted only to route requests into the S08
    boundary rejection path; the resulting ``ValueError`` is intentionally not
    caught or converted into a successful workflow.
    """
    navigation_diagnostics = tuple(
        RLMWorkflowDiagnostic(
            phase="validate_navigation",
            status="blocked",
            message=message,
        )
        for message in document.validate_navigation()
    )
    validation_step = RLMWorkflowStep(
        phase="validate_navigation",
        status="blocked" if navigation_diagnostics else "ok",
        node_id=document.root.id,
        counts=(("diagnostics", len(navigation_diagnostics)), ("nodes", len(document.nodes))),
        diagnostics=navigation_diagnostics,
        boundary_valid=False if navigation_diagnostics else None,
    )
    if navigation_diagnostics:
        return RLMWorkflowResult(
            status="blocked",
            navigation_valid=False,
            boundary_valid=False,
            context=None,
            trajectory=(validation_step,),
            diagnostics=navigation_diagnostics,
        )

    walked_nodes = tuple(document.walk_next())
    node_ids = tuple(node.id for node in walked_nodes)
    chunk_ids = tuple(chunk.id for chunk in chunks)
    evidence_path_ids = tuple(evidence_path_id(path) for path in evidence_paths)

    trajectory: list[RLMWorkflowStep] = [
        validation_step,
        RLMWorkflowStep(
            phase="walk_next",
            status="ok",
            node_id=document.root.id,
            path_node_ids=node_ids,
            counts=(("nodes", len(node_ids)),),
        ),
    ]

    for node in walked_nodes:
        children = tuple(child.id for child in document.children_of(node.id))
        trajectory.append(
            RLMWorkflowStep(
                phase="children",
                status="ok",
                node_id=node.id,
                child_node_ids=children,
                counts=(("children", len(children)),),
            )
        )
        path_node_ids = tuple(document.path_to(node.id))
        trajectory.append(
            RLMWorkflowStep(
                phase="path",
                status="ok" if path_node_ids else "warning",
                node_id=node.id,
                path_node_ids=path_node_ids,
                counts=(("path_nodes", len(path_node_ids)),),
            )
        )

    diagnostics: list[RLMWorkflowDiagnostic] = []
    diagnostics.extend(_chunk_diagnostics(document, chunks))
    diagnostics.extend(_evidence_diagnostics(document, chunks, evidence_paths))

    trajectory.append(
        RLMWorkflowStep(
            phase="chunks",
            status="warning" if any(d.phase == "chunks" for d in diagnostics) else "ok",
            semantic_chunk_ids=chunk_ids,
            counts=(("chunks", len(chunk_ids)),),
            diagnostics=tuple(d for d in diagnostics if d.phase == "chunks"),
        )
    )
    trajectory.append(
        RLMWorkflowStep(
            phase="evidence",
            status="warning" if any(d.phase == "evidence" for d in diagnostics) else "ok",
            evidence_path_ids=evidence_path_ids,
            counts=(("evidence_paths", len(evidence_path_ids)),),
            diagnostics=tuple(d for d in diagnostics if d.phase == "evidence"),
            boundary_valid=not diagnostics,
        )
    )

    context = RLMNavigationContext(
        paper_id=document.paper_id,
        root_node_id=document.root.id,
        node_ids=node_ids,
        semantic_chunk_ids=chunk_ids,
        evidence_path_ids=evidence_path_ids,
        counts=(
            ("nodes", len(node_ids)),
            ("chunks", len(chunk_ids)),
            ("evidence_paths", len(evidence_path_ids)),
            ("diagnostics", len(diagnostics)),
        ),
    )
    if diagnostics:
        trajectory.append(
            RLMWorkflowStep(
                phase="draft",
                status="warning",
                counts=context.counts,
                boundary_valid=False,
            )
        )
        return RLMWorkflowResult(
            status="warning",
            navigation_valid=True,
            boundary_valid=False,
            context=context,
            trajectory=tuple(trajectory),
            diagnostics=tuple(diagnostics),
        )

    boundary_output = _run_extraction_boundary(
        extractor,
        context=context,
        optimizer_config=optimizer_config,
    )
    draft_diagnostics = _draft_diagnostics(boundary_output)
    diagnostics.extend(draft_diagnostics)
    draft_step = _draft_step(context, boundary_output, draft_diagnostics)
    trajectory.append(draft_step)

    return RLMWorkflowResult(
        status="ok" if draft_step.status == "ok" else "warning",
        navigation_valid=True,
        boundary_valid=draft_step.boundary_valid,
        context=context,
        trajectory=tuple(trajectory),
        diagnostics=tuple(diagnostics),
        draft=boundary_output.patch if boundary_output is not None else None,
        boundary_output=boundary_output,
    )


def _run_extraction_boundary(
    extractor: BaselineDspyExtractionModule | None,
    *,
    context: RLMNavigationContext,
    optimizer_config: Mapping[str, Any] | None,
) -> DspyExtractionOutput | None:
    if extractor is None:
        return None

    boundary_input = DspyExtractionInput(
        paper_id=context.paper_id,
        expected_evidence_path_ids=frozenset(context.evidence_path_ids),
        baseline_context={
            "paper_id": context.paper_id,
            "root_node_id": context.root_node_id,
            "page_index_node_ids": context.node_ids,
            "semantic_chunk_ids": context.semantic_chunk_ids,
            "evidence_path_ids": context.evidence_path_ids,
            "counts": dict(context.counts),
            "route_status": context.route_status,
        },
        optimizer_config=optimizer_config,
    )
    return extractor.forward(boundary_input)


def _draft_step(
    context: RLMNavigationContext,
    boundary_output: DspyExtractionOutput | None,
    draft_diagnostics: tuple[RLMWorkflowDiagnostic, ...],
) -> RLMWorkflowStep:
    if boundary_output is None:
        return RLMWorkflowStep(
            phase="draft",
            status="ok",
            counts=(*context.counts, ("extractor_calls", 0)),
            diagnostics=draft_diagnostics,
            boundary_valid=True,
        )

    boundary_valid = (
        boundary_output.schema_valid
        and boundary_output.groundedness_valid
        and boundary_output.boundary_valid
        and not boundary_output.optimizer_enabled
        and not boundary_output.boundary_diagnostics
    )
    return RLMWorkflowStep(
        phase="draft",
        status="ok" if boundary_valid else "warning",
        evidence_path_ids=tuple(
            boundary_output.groundedness_diagnostics.get("derived_evidence_path_ids", ())
        ),
        counts=(
            *context.counts,
            ("extractor_calls", 1),
            ("schema_diagnostics", boundary_output.schema_diagnostic_count),
            ("boundary_diagnostics", len(boundary_output.boundary_diagnostics)),
            (
                "missing_expected_evidence_paths",
                len(
                    boundary_output.groundedness_diagnostics.get(
                        "missing_expected_evidence_path_ids", ()
                    )
                ),
            ),
            (
                "missing_evidence_path_drafts",
                len(
                    boundary_output.groundedness_diagnostics.get(
                        "missing_evidence_path_draft_ids", ()
                    )
                ),
            ),
        ),
        diagnostics=draft_diagnostics,
        boundary_valid=boundary_valid,
        schema_valid=boundary_output.schema_valid,
        groundedness_valid=boundary_output.groundedness_valid,
        optimizer_enabled=boundary_output.optimizer_enabled,
    )


def _draft_diagnostics(
    boundary_output: DspyExtractionOutput | None,
) -> tuple[RLMWorkflowDiagnostic, ...]:
    if boundary_output is None:
        return ()

    diagnostics: list[RLMWorkflowDiagnostic] = []
    if boundary_output.optimizer_enabled:
        diagnostics.append(
            RLMWorkflowDiagnostic(
                phase="draft",
                status="warning",
                message="optimizer_enabled",
            )
        )
    if not boundary_output.schema_valid:
        diagnostics.append(
            RLMWorkflowDiagnostic(
                phase="draft",
                status="warning",
                message="schema_invalid",
            )
        )
    if not boundary_output.groundedness_valid:
        diagnostics.append(
            RLMWorkflowDiagnostic(
                phase="draft",
                status="warning",
                message="groundedness_invalid",
            )
        )
    if not boundary_output.boundary_valid:
        diagnostics.append(
            RLMWorkflowDiagnostic(
                phase="draft",
                status="warning",
                message="boundary_invalid",
            )
        )
    diagnostics.extend(
        RLMWorkflowDiagnostic(
            phase="draft",
            status="warning",
            message=diagnostic,
        )
        for diagnostic in boundary_output.boundary_diagnostics
    )
    return tuple(diagnostics)


def _chunk_diagnostics(
    document: PageIndexDocument,
    chunks: list[SemanticChunk],
) -> list[RLMWorkflowDiagnostic]:
    diagnostics: list[RLMWorkflowDiagnostic] = []
    for chunk in chunks:
        if chunk.paper_id != document.paper_id:
            diagnostics.append(
                RLMWorkflowDiagnostic(
                    phase="chunks",
                    status="warning",
                    message=(
                        f"SemanticChunk {chunk.id} paper_id {chunk.paper_id} does not match "
                        f"document paper_id {document.paper_id}"
                    ),
                    semantic_chunk_id=chunk.id,
                )
            )
        node = document.node_by_id(chunk.page_index_node_id)
        if node is None:
            diagnostics.append(
                RLMWorkflowDiagnostic(
                    phase="chunks",
                    status="warning",
                    message=f"SemanticChunk {chunk.id} references missing PageIndexNode {chunk.page_index_node_id}",
                    node_id=chunk.page_index_node_id,
                    semantic_chunk_id=chunk.id,
                )
            )
        elif list(chunk.page_index_path) != list(node.path):
            diagnostics.append(
                RLMWorkflowDiagnostic(
                    phase="chunks",
                    status="warning",
                    message=(
                        f"SemanticChunk {chunk.id} page_index_path {'/'.join(chunk.page_index_path)} "
                        f"does not match PageIndexNode path {'/'.join(node.path)}"
                    ),
                    node_id=chunk.page_index_node_id,
                    semantic_chunk_id=chunk.id,
                )
            )
    return diagnostics


def _evidence_diagnostics(
    document: PageIndexDocument,
    chunks: list[SemanticChunk],
    evidence_paths: list[EvidencePath],
) -> list[RLMWorkflowDiagnostic]:
    if not evidence_paths:
        return [
            RLMWorkflowDiagnostic(
                phase="evidence",
                status="warning",
                message="no evidence paths provided",
            )
        ]

    diagnostics: list[RLMWorkflowDiagnostic] = []
    for path in evidence_paths:
        for message in validate_evidence_path(path, document, chunks):
            diagnostics.append(
                RLMWorkflowDiagnostic(
                    phase="evidence",
                    status="warning",
                    message=message,
                    node_id=path.page_index_node_id,
                    semantic_chunk_id=path.semantic_chunk_id,
                    evidence_path_id=evidence_path_id(path),
                )
            )
    return diagnostics


__all__ = [
    "RLMNavigationContext",
    "RLMWorkflowDiagnostic",
    "RLMWorkflowResult",
    "RLMWorkflowStep",
    "run_document_workflow",
]
