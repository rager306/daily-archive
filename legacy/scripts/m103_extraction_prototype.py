#!/usr/bin/env python3
"""M103 S03 extraction prototype: run the typed pipeline against labeled chunks
via the real MiniMax API (ADR-029, ADR-033 Step 10).

This is a PROTOTYPE script (artifacts-producing, not a production module). It:

* Loads .env, loads the 5 hand-labeled golden chunks (T01 fixtures) that stand
  in for arXiv:2605.18747 chunk text (paper-level parsing is Phase 3+).
* Builds a real ``llm_client`` backed by MiniMax's Anthropic-compatible API
  (base_url ``https://api.minimax.io/anthropic`` -> ``/anthropic/v1/messages``;
  the single ``MINIMAX_API_KEY`` value sent as ``X-Api-Key``; forced tool calls
  with ``input_schema`` for schema-validated structured output — the canonical
  path per the ``minimax-safe-helper`` skill and ``MiniMaxSummarizer``).
* Runs the paper pipeline (``build_paper_pipeline``) with the real client
  injected into ``CoreEntityExtractor`` and ``RelationTypeClassifier``,
  statistical-first (YAKE pre-processing before every LLM call), rate-limited
  via a simple token-plan sleep.
* Scores extracted :class:`TypedEntity` / :class:`TypedRelation` against the
  golden labels (precision/recall by canonical-name and relation-type+endpoints).
* Writes fail-closed artifacts (``safety_flags.import_eligible = False``) to
  ``artifacts/m103-extraction-prototype/``: the ExtractionPatch per chunk, a
  merged summary, and a metrics JSON. NO graph writes, NO import authorization.

Usage::

    uv run python scripts/m103_extraction_prototype.py [--max-chunks N] [--dry-run]

``--dry-run`` runs the CPU-lane stages only (no LLM call) to verify the wiring.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_DIR = ROOT / "artifacts" / "m103-extraction-prototype"
FIXTURES = ROOT / "tests" / "fixtures" / "m103_extraction" / "labeled_chunks.json"

# Rate limit: MiniMax token plan. Conservative — one LLM call per stage per
# chunk, with a short sleep between calls. Tuned to stay well under plan limits
# for a 5-chunk prototype (ADR-027 §5 Phase 2 simple LLM-lane check).
LLM_CALL_DELAY_SECONDS = 2.0


def _load_env() -> None:
    """Load .env so MiniMax keys are present for this CLI process."""
    from research_graph.infrastructure.retrieval.embedder import load_embedder_env_config

    load_embedder_env_config().apply_to_environ()


def _require_key() -> str:
    """Return the canonical MiniMax key (MINIMAX_API_KEY; ANTHROPIC_API_KEY is an alias)."""
    key = os.environ.get("MINIMAX_API_KEY") or os.environ.get("ANTHROPIC_API_KEY")
    if not key:
        sys.stderr.write("ERROR: no MiniMax API key found (need MINIMAX_API_KEY in .env)\n")
        raise SystemExit(2)
    return key


def make_minimax_extraction_client(api_key: str, model: str = "MiniMax-M2.7-highspeed"):
    """Build a real ``llm_client`` for the pipeline's LLM stages.

    Canonical path per the minimax-safe-helper skill: Anthropic-compatible API,
    ``base_url=https://api.minimax.io/anthropic`` (the SDK appends
    ``/v1/messages``), the single ``MINIMAX_API_KEY`` value sent as ``X-Api-Key``
    via the Anthropic SDK's ``api_key`` arg, and forced tool calls with
    ``input_schema`` for schema-validated structured output (NOT prompt-only
    JSON — that is an anti-pattern in the skill). Local schema validation backs
    the tool output. Failures return an empty dict (fail-closed: zero drafts).
    """
    import anthropic

    client = anthropic.Anthropic(api_key=api_key, base_url="https://api.minimax.io/anthropic")

    def _call(prompt: str, snapshot: dict[str, Any]) -> dict[str, Any]:
        kind = snapshot.get("extraction_kind", "entities")
        tool_name, input_schema = _tool_schema_for(kind)
        try:
            response = client.messages.create(
                model=model,
                max_tokens=1024,
                temperature=0.2,  # skill: (0.0, 1.0]
                messages=[{"role": "user", "content": prompt}],
                tools=[
                    {
                        "name": tool_name,
                        "description": f"Return extracted {kind} as JSON.",
                        "input_schema": input_schema,
                    }
                ],
                tool_choice={"type": "tool", "name": tool_name},
            )
        except Exception as exc:  # noqa: BLE001 — fail-closed, log sanitized
            sys.stderr.write(f"[minimax client] call failed ({type(exc).__name__}): {exc}\n")
            return {}
        # Skill: preserve full assistant response incl. thinking; for single-call
        # extraction we only consume the tool_use block.
        for block in response.content:
            if getattr(block, "type", None) == "tool_use":
                # pyrefly: ignore [missing-attribute]
                data = block.input  # ty:ignore[unresolved-attribute]
                return data if isinstance(data, dict) else {}
        return {}

    return _call


def _tool_schema_for(kind: str) -> tuple[str, dict[str, Any]]:
    """Return (tool_name, input_schema) for the extraction kind (Anthropic forced tool)."""
    if kind == "relations":
        return (
            "emit_relations",
            {
                "type": "object",
                "properties": {
                    "relations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "relation_type": {"type": "string"},
                                "from_name": {"type": "string"},
                                "to_name": {"type": "string"},
                                "confidence": {"type": "number"},
                            },
                            "required": ["relation_type", "from_name", "to_name", "confidence"],
                        },
                    }
                },
                "required": ["relations"],
            },
        )
    return (
        "emit_entities",
        {
            "type": "object",
            "properties": {
                "entities": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "entity_type": {"type": "string"},
                            "canonical_name": {"type": "string"},
                            "confidence": {"type": "number"},
                            "evidence_hint": {"type": "string"},
                        },
                        "required": ["entity_type", "canonical_name", "confidence"],
                    },
                }
            },
            "required": ["entities"],
        },
    )


# ── Prompt builders (statistical-first: keywords embedded) ───────────────────


def _entity_prompt(text: str, keywords: list[str]) -> str:
    kw = ", ".join(keywords[:15])
    return (
        "Extract the core technical entities (methods, datasets, metrics, tasks, "
        "baselines, problems) from the following paper chunk. Use the statistical "
        f"keyword context as grounding (do not invent entities outside the text).\n\n"
        f"Statistical keywords: {kw}\n\nChunk:\n{text}"
    )


def _relation_prompt(text: str, candidates: list[dict[str, Any]]) -> str:
    cand = json.dumps(candidates[:10])
    return (
        "Classify the relations between extracted entities into ONE of the 27 typed "
        "relations (BUILDS_ON, IMPLEMENTS, EXTENDS, SOLVES, TARGETS, CAUSES, ENABLES, "
        "INHIBITS, CONSISTS_OF, REQUIRES, DERIVED_FROM, HAS_LIMITATION, SUBSET_OF, "
        "CITES, SUPPORTS, CONTRASTS, etc.). Drop any relation whose type is not one "
        "of the 27. Only relate entities actually present in the text.\n\n"
        f"Candidate pairs: {cand}\n\nChunk:\n{text}"
    )


# ── Pipeline runner with rate limiting ───────────────────────────────────────


def run_chunk(
    chunk: dict[str, Any],
    client,
    *,
    rate_limit_delay: float = LLM_CALL_DELAY_SECONDS,
) -> dict[str, Any]:
    """Run the paper pipeline on one chunk with the real LLM client."""
    from dataclasses import replace

    from research_graph.application.primitives import (
        BinaryRelationDetector,
        CoreEntityExtractor,
        RelationTypeClassifier,
        StatisticalPreProcessor,
    )
    from research_graph.application.types import PipelineContext

    text = chunk["text"]
    source_id = f"arxiv:2605.18747:{chunk['chunk_id']}"

    # Stage 1: statistical (CPU, no LLM). min=1 because chunk-level input is a
    # single text part, so any co-occurring pair appears once; the LLM classifier
    # (not the count threshold) is the quality gate.
    ctx0 = replace(PipelineContext(source_id=source_id), stage_outputs={"text_parts": [text]})
    ctx1 = StatisticalPreProcessor(co_occurrence_min=1).run(ctx0)
    # pyrefly: ignore [missing-attribute]
    keywords = [k for k, _ in ctx1.statistical_context.keywords]  # ty:ignore[unresolved-attribute]

    # Stage 2: core entities (LLM, rate-limited)
    entity_client = _wrap_with_kind(client, "entities")
    core = CoreEntityExtractor(llm_client=entity_client)
    time.sleep(rate_limit_delay)
    ctx2 = core.run(ctx1)

    # Stage 3: binary relations (CPU)
    ctx3 = BinaryRelationDetector(min_co_occurrence=1).run(ctx1)

    # Stage 4: typed relations (LLM, rate-limited)
    relation_client = _wrap_with_kind(client, "relations")
    clf = RelationTypeClassifier(llm_client=relation_client)
    time.sleep(rate_limit_delay)
    # Inject candidate dump so the LLM sees the pairs (via snapshot)
    ctx4_pre = replace(
        ctx3,
        stage_outputs={
            **ctx3.stage_outputs,
            "core_entity_extractor": ctx2.stage_outputs["core_entity_extractor"],
        },
    )
    ctx4 = clf.run(ctx4_pre)

    return {
        "chunk_id": chunk["chunk_id"],
        "source_id": source_id,
        "keywords": keywords[:10],
        "patch": _patch_to_dict(
            ctx4.stage_outputs.get("relation_type_classifier")
            or ctx2.stage_outputs["core_entity_extractor"]
        ),
        "golden": {
            "entities": chunk.get("expected_entities", []),
            "relations": chunk.get("expected_relations", []),
        },
    }


def _wrap_with_kind(client, kind: str):
    """Wrap client so the snapshot carries extraction_kind for tool selection."""

    def _wrapped(prompt: str, snapshot: dict[str, Any]) -> dict[str, Any]:
        snap = {**snapshot, "extraction_kind": kind}
        return client(prompt, snap)

    return _wrapped


def _patch_to_dict(patch) -> dict[str, Any]:
    from research_graph.domain.schema import ExtractionPatch

    if not isinstance(patch, ExtractionPatch):
        return {"entities": [], "relations": [], "safety_flags": {"import_eligible": False}}
    return {
        "entities": [
            {
                "entity_id": e.entity_id,
                "entity_type": e.entity_type,
                "canonical_name": e.canonical_name,
                "confidence": e.confidence,
            }
            for e in patch.entities
        ],
        "relations": [
            {
                "relation_id": r.relation_id,
                "relation_type": r.relation_type,
                "from_entity_id": r.from_entity_id,
                "to_entity_id": r.to_entity_id,
                "confidence": r.confidence,
            }
            for r in patch.relations
        ],
        "safety_flags": dict(patch.safety_flags),
    }


# ── Scoring (precision/recall vs golden) ─────────────────────────────────────


def _norm_name(name: str) -> str:
    """Normalize an entity name for matching: lowercase, separators collapsed."""
    return name.lower().replace("-", " ").replace("_", " ").strip()


def _soft_set_match(extracted: set[tuple], golden: set[tuple]) -> int:
    """Count extracted relation tuples matching golden (soft, on normalized names).

    A triple matches if relation_type is equal and both endpoint names soft-match
    (exact, or one contains the other) a golden triple's endpoints.
    """
    matched = 0
    golden_by_type: dict[str, list[tuple[str, str]]] = {}
    for rt, a, b in golden:
        golden_by_type.setdefault(rt, []).append((a, b))
    for rt, a, b in extracted:
        for ga, gb in golden_by_type.get(rt, []):
            if _name_pair_match(a, ga) and _name_pair_match(b, gb):
                matched += 1
                break
    return matched


def _name_pair_match(x: str, g: str) -> bool:
    return x == g or x in g or g in x


def score_chunk(result: dict[str, Any]) -> dict[str, Any]:
    golden_names = {e["canonical_name"].lower() for e in result["golden"]["entities"]}
    golden_aliases = set()
    for e in result["golden"]["entities"]:
        golden_aliases.add(e["canonical_name"].lower())
        for alias in e.get("aliases", []):
            golden_aliases.add(alias.lower())
    extracted_names = {e["canonical_name"].lower() for e in result["patch"]["entities"]}

    # Soft match: extracted matches golden if exact, alias, or one contains the other
    # (handles "Sparse Routing Attention (SRA)" vs "Sparse Routing Attention").
    def _soft_match(extracted: str, golden_set: set[str]) -> bool:
        if extracted in golden_set:
            return True
        for g in golden_set:
            if extracted in g or g in extracted:
                return True
        return False

    tp = sum(1 for ex in extracted_names if _soft_match(ex, golden_aliases))
    precision = tp / len(extracted_names) if extracted_names else 0.0
    recall = tp / len(golden_names) if golden_names else 0.0
    golden_rel = {
        (r["relation_type"], _norm_name(r["from"]), _norm_name(r["to"]))
        for r in result["golden"]["relations"]
    }
    # Extracted relations carry ids, not names; recover names from the slug tail
    # and normalize so "sparse-routing-attention" matches "Sparse Routing Attention".
    extracted_rel = set()
    for r in result["patch"]["relations"]:
        from_name = _norm_name(r["from_entity_id"].rsplit(":", 1)[-1])
        to_name = _norm_name(r["to_entity_id"].rsplit(":", 1)[-1])
        extracted_rel.add((r["relation_type"], from_name, to_name))
    if golden_rel and extracted_rel:
        tp_r = _soft_set_match(extracted_rel, golden_rel)
        rel_precision = tp_r / len(extracted_rel) if extracted_rel else 0.0
        rel_recall = tp_r / len(golden_rel) if golden_rel else 0.0
    else:
        rel_precision = rel_recall = 0.0
    return {
        "entity_precision": round(precision, 3),
        "entity_recall": round(recall, 3),
        "relation_precision": round(rel_precision, 3),
        "relation_recall": round(rel_recall, 3),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--max-chunks", type=int, default=5, help="max chunks to process")
    parser.add_argument("--dry-run", action="store_true", help="CPU stages only, no LLM call")
    parser.add_argument(
        "--delay", type=float, default=LLM_CALL_DELAY_SECONDS, help="LLM call delay"
    )
    args = parser.parse_args()

    _load_env()
    ARTIFACT_DIR.mkdir(parents=True, exist_ok=True)
    fixtures = json.loads(FIXTURES.read_text())
    chunks = fixtures["chunks"][: args.max_chunks]

    client = None
    if not args.dry_run:
        key = _require_key()
        client = make_minimax_extraction_client(key)

    results = []
    for chunk in chunks:
        if args.dry_run:
            # CPU-only: stub the LLM (no network)
            from dataclasses import replace

            from research_graph.application.primitives import (
                BinaryRelationDetector,
                CoreEntityExtractor,
                StatisticalPreProcessor,
            )
            from research_graph.application.types import PipelineContext

            ctx = replace(
                PipelineContext(source_id=f"arxiv:2605.18747:{chunk['chunk_id']}"),
                stage_outputs={"text_parts": [chunk["text"]]},
            )
            ctx = StatisticalPreProcessor(co_occurrence_min=2).run(ctx)
            ctx = CoreEntityExtractor().run(ctx)  # stubbed -> empty
            ctx = BinaryRelationDetector().run(ctx)
            results.append(
                {
                    "chunk_id": chunk["chunk_id"],
                    "dry_run": True,
                    # pyrefly: ignore [missing-attribute]
                    "keywords": [k for k, _ in ctx.statistical_context.keywords][:10],  # ty:ignore[unresolved-attribute]
                    "patch": _patch_to_dict(ctx.stage_outputs["core_entity_extractor"]),
                    "golden": {
                        "entities": chunk.get("expected_entities", []),
                        "relations": chunk.get("expected_relations", []),
                    },
                    "scores": {
                        "entity_precision": 0.0,
                        "entity_recall": 0.0,
                        "relation_precision": 0.0,
                        "relation_recall": 0.0,
                    },
                }
            )
            continue

        sys.stdout.write(f"[prototype] processing {chunk['chunk_id']}...\n")
        res = run_chunk(chunk, client, rate_limit_delay=args.delay)
        res["scores"] = score_chunk(res)
        results.append(res)

    # Aggregate metrics
    if not args.dry_run and results:
        n = len(results)
        agg = {
            "chunks_processed": n,
            "entity_precision_mean": round(
                # pyrefly: ignore [bad-index]
                sum(r["scores"]["entity_precision"] for r in results) / n,  # ty:ignore[invalid-argument-type, not-subscriptable]
                3,  # pyrefly: ignore[bad-assignment]
            ),
            "entity_recall_mean": round(sum(r["scores"]["entity_recall"] for r in results) / n, 3),  # pyrefly: ignore [bad-assignment, bad-index]  # ty:ignore[invalid-argument-type, not-subscriptable]
            "relation_precision_mean": round(
                # pyrefly: ignore [bad-index]
                sum(r["scores"]["relation_precision"] for r in results) / n,  # ty:ignore[invalid-argument-type, not-subscriptable]
                3,  # pyrefly: ignore[bad-assignment]
            ),
            "relation_recall_mean": round(
                # pyrefly: ignore [bad-index]
                sum(r["scores"]["relation_recall"] for r in results) / n,  # ty:ignore[invalid-argument-type, not-subscriptable]
                3,  # pyrefly: ignore[bad-assignment]
            ),
        }
    else:
        agg = {"chunks_processed": len(results), "dry_run": True}

    payload = {
        "prototype_version": "m103.extraction.prototype.v1",
        "anchor_paper": "arxiv:2605.18747",
        "note": "Chunk-level prototype (5 labeled golden chunks as proxy for paper text; "
        "paper-level parsing is Phase 3+). Fail-closed: safety_flags.import_eligible=False everywhere.",
        "model": "MiniMax-M2.7-highspeed (Anthropic-compatible path)",
        "aggregate": agg,
        "all_safety_flags_false": all(
            not r.get("patch", {}).get("safety_flags", {}).get("import_eligible", False)  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
            for r in results
        ),
        "results": results,
    }
    out = ARTIFACT_DIR / ("prototype-dry-run.json" if args.dry_run else "prototype-summary.json")
    out.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n")

    sys.stdout.write(f"\nprototype complete → {out}\n")
    sys.stdout.write(f"aggregate: {json.dumps(agg)}\n")
    sys.stdout.write(
        f"all safety_flags.import_eligible=False: {payload['all_safety_flags_false']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
