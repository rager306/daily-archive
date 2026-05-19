"""Retrieval validation fixture for S06 trusted candidate claims.

This module validates and loads the S06 `persisted-candidate-claims.jsonl`
artifact. It intentionally avoids raw paper text, embeddings, LLM calls,
LadybugDB writes, and broad corpus retrieval. Later S07 tasks add deterministic
exact-ID retrieval over the loaded records.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from arxiv_archive.graph_readiness import to_redacted_dict

EXPECTED_SCOPE = "validation_subset"
EXPECTED_SCHEMA = "s06-persisted-candidate-claim.v1"
FORBIDDEN_RAW_TEXT_FIELDS = frozenset(
    {
        "text",
        "claim_text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
    }
)


@dataclass(frozen=True)
class RetrievalFixtureRecord:
    """One S06 trusted candidate claim available for exact-ID retrieval validation."""

    paper_id: str
    candidate_id: str
    chunk_id: str
    source_artifact: str
    finding_codes: list[str]
    persisted_scope: str
    entry_id: str
    claim_draft_id: str
    raw_text_included: bool = False
    embeddings_included: bool = False


@dataclass(frozen=True)
class FixtureRefusal:
    """Reason a persisted-candidate record was rejected by the retrieval fixture loader."""

    line_number: int
    reason: str
    paper_id: str | None = None
    candidate_id: str | None = None
    chunk_id: str | None = None


@dataclass(frozen=True)
class RetrievalFixtureLoadResult:
    """Accepted fixture records plus refusal diagnostics."""

    records: list[RetrievalFixtureRecord]
    refusals: list[FixtureRefusal]

    @property
    def counts(self) -> dict[str, int]:
        by_reason: dict[str, int] = {}
        for refusal in self.refusals:
            by_reason[refusal.reason] = by_reason.get(refusal.reason, 0) + 1
        return {
            "accepted_records": len(self.records),
            "refused_records": len(self.refusals),
            **{f"refused_{reason}": count for reason, count in sorted(by_reason.items())},
            "raw_text_included": 0,
            "embeddings_included": 0,
        }


@dataclass(frozen=True)
class RetrievalValidationResult:
    """Deterministic exact-ID retrieval validation result."""

    records: list[RetrievalFixtureRecord]
    events: list[dict[str, Any]]
    summary: dict[str, Any]


def load_retrieval_fixture(path: Path) -> RetrievalFixtureLoadResult:
    """Load S06 persisted candidate claims with strict redaction/provenance validation."""
    records: list[RetrievalFixtureRecord] = []
    refusals: list[FixtureRefusal] = []
    for line_number, line in enumerate(Path(path).read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            refusals.append(FixtureRefusal(line_number=line_number, reason="invalid_json"))
            continue
        if not isinstance(payload, dict):
            refusals.append(FixtureRefusal(line_number=line_number, reason="not_json_object"))
            continue
        reason = _record_refusal_reason(payload)
        if reason is not None:
            refusals.append(_refusal_from_payload(line_number, payload, reason))
            continue
        records.append(_record_from_payload(payload))
    return RetrievalFixtureLoadResult(records=records, refusals=refusals)


def run_retrieval_validation(claims_path: Path, *, output_dir: Path | None = None) -> RetrievalValidationResult:
    """Run deterministic exact-ID retrieval validation over S06 trusted records."""
    load_result = load_retrieval_fixture(claims_path)
    if load_result.refusals:
        summary = _retrieval_summary(records=[], events=[], load_refusals=len(load_result.refusals))
        return RetrievalValidationResult(records=[], events=[], summary=summary)

    records = load_result.records
    events: list[dict[str, Any]] = []
    for record in records:
        events.extend(_query_events_for_record(record, records))
    summary = _retrieval_summary(records=records, events=events, load_refusals=0)
    if output_dir is not None:
        output_dir = Path(output_dir)
        output_dir.mkdir(parents=True, exist_ok=True)
        (output_dir / "retrieval-validation-results.json").write_text(
            json.dumps(to_redacted_dict(summary), indent=2, sort_keys=True),
            encoding="utf-8",
        )
        (output_dir / "retrieval-validation-events.jsonl").write_text(
            "".join(json.dumps(to_redacted_dict(event), sort_keys=True) + "\n" for event in events),
            encoding="utf-8",
        )
    return RetrievalValidationResult(records=records, events=events, summary=summary)


def fixture_load_to_dict(result: RetrievalFixtureLoadResult) -> dict[str, Any]:
    """Serialize loader diagnostics without raw text."""
    return {
        "schema_version": "s07-retrieval-fixture-load.v1",
        "counts": result.counts,
        "records": [record.__dict__ for record in result.records],
        "refusals": [refusal.__dict__ for refusal in result.refusals],
        "raw_text_included": False,
        "embeddings_included": False,
    }


def _query_events_for_record(record: RetrievalFixtureRecord, records: list[RetrievalFixtureRecord]) -> list[dict[str, Any]]:
    return [
        _query_event("paper_id", record.paper_id, [item for item in records if item.paper_id == record.paper_id]),
        _query_event("candidate_id", record.candidate_id, [item for item in records if item.candidate_id == record.candidate_id]),
        _query_event("chunk_id", record.chunk_id, [item for item in records if item.chunk_id == record.chunk_id]),
        _query_event(
            "source_artifact",
            record.source_artifact,
            [item for item in records if item.source_artifact == record.source_artifact],
        ),
    ]


def _query_event(query_type: str, query_value: str, matches: list[RetrievalFixtureRecord]) -> dict[str, Any]:
    return {
        "event": "retrieval_validation.query",
        "query_type": query_type,
        "query_value": query_value,
        "hit": bool(matches),
        "hit_count": len(matches),
        "returned_candidate_ids": [record.candidate_id for record in matches],
        "returned_chunk_ids": [record.chunk_id for record in matches],
        "returned_paper_ids": [record.paper_id for record in matches],
        "raw_text_included": False,
        "embeddings_included": False,
    }


def _retrieval_summary(
    *,
    records: list[RetrievalFixtureRecord],
    events: list[dict[str, Any]],
    load_refusals: int,
) -> dict[str, Any]:
    total_queries = len(events)
    hits = sum(1 for event in events if event["hit"])
    record_count = len(records)
    unique_papers = len({record.paper_id for record in records})
    unique_candidates = len({record.candidate_id for record in records})
    unique_chunks = len({record.chunk_id for record in records})
    unique_sources = len({record.source_artifact for record in records})
    return {
        "schema_version": "s07-retrieval-validation-results.v1",
        "retrieval_scope": "s06_validation_subset_exact_id",
        "record_count": record_count,
        "load_refusals": load_refusals,
        "query_count": total_queries,
        "hit_count": hits,
        "exact_match_rate": hits / total_queries if total_queries else 0.0,
        "candidate_coverage": unique_candidates / record_count if record_count else 0.0,
        "chunk_coverage": unique_chunks / record_count if record_count else 0.0,
        "paper_coverage": unique_papers / record_count if record_count else 0.0,
        "source_artifact_coverage": unique_sources / record_count if record_count else 0.0,
        "raw_text_included": False,
        "embeddings_included": False,
        "llm_used": False,
        "ladybugdb_used": False,
        "dspy_used": False,
        "limitations": [
            "exact_id_metadata_retrieval_only",
            "no_semantic_similarity",
            "no_entity_or_relation_retrieval",
            "no_broad_corpus_claim",
        ],
    }


def _record_refusal_reason(payload: dict[str, Any]) -> str | None:
    forbidden_present = FORBIDDEN_RAW_TEXT_FIELDS & set(payload)
    if forbidden_present:
        return f"forbidden_field_{sorted(forbidden_present)[0]}"
    if payload.get("schema_version") != EXPECTED_SCHEMA:
        return "unexpected_schema_version"
    if payload.get("persisted_scope") != EXPECTED_SCOPE:
        return "unexpected_persisted_scope"
    if payload.get("persisted") is not True:
        return "not_persisted"
    if payload.get("raw_text_included") is not False or payload.get("claim_text_included") is not False:
        return "raw_text_flag_not_false"
    if payload.get("embeddings_included") is not False:
        return "embeddings_flag_not_false"
    for field in ("paper_id", "candidate_id", "chunk_id", "source_artifact", "entry_id", "claim_draft_id"):
        if not payload.get(field):
            return f"missing_{field}"
    finding_codes = payload.get("finding_codes")
    if not isinstance(finding_codes, list) or not finding_codes:
        return "missing_finding_codes"
    return None


def _record_from_payload(payload: dict[str, Any]) -> RetrievalFixtureRecord:
    return RetrievalFixtureRecord(
        paper_id=str(payload["paper_id"]),
        candidate_id=str(payload["candidate_id"]),
        chunk_id=str(payload["chunk_id"]),
        source_artifact=str(payload["source_artifact"]),
        finding_codes=[str(code) for code in payload["finding_codes"]],
        persisted_scope=str(payload["persisted_scope"]),
        entry_id=str(payload["entry_id"]),
        claim_draft_id=str(payload["claim_draft_id"]),
        raw_text_included=False,
        embeddings_included=False,
    )


def _refusal_from_payload(line_number: int, payload: dict[str, Any], reason: str) -> FixtureRefusal:
    return FixtureRefusal(
        line_number=line_number,
        reason=reason,
        paper_id=_string_or_none(payload.get("paper_id")),
        candidate_id=_string_or_none(payload.get("candidate_id")),
        chunk_id=_string_or_none(payload.get("chunk_id")),
    )


def _string_or_none(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value)
    return text if text else None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Load and validate S06 trusted-candidate retrieval fixture.")
    parser.add_argument("--claims", required=True, type=Path)
    parser.add_argument("--output", required=False, type=Path)
    parser.add_argument("--validate-retrieval-output-dir", required=False, type=Path)
    args = parser.parse_args(argv)
    if args.validate_retrieval_output_dir is not None:
        validation = run_retrieval_validation(args.claims, output_dir=args.validate_retrieval_output_dir)
        sys.stdout.write(json.dumps(to_redacted_dict(validation.summary), indent=2, sort_keys=True))
        sys.stdout.write("\n")
        return 0 if validation.summary["load_refusals"] == 0 else 1

    result = load_retrieval_fixture(args.claims)
    payload = to_redacted_dict(fixture_load_to_dict(result))
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    sys.stdout.write(json.dumps(payload["counts"], indent=2, sort_keys=True))
    sys.stdout.write("\n")
    return 0 if not result.refusals else 1


if __name__ == "__main__":
    raise SystemExit(main())
