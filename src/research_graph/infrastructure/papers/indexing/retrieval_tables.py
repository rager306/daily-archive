"""Metadata-only retrieval-unit and table-candidate benchmark manifests.

This module composes PageIndex, asset, and link/dedup provenance into a
CPU-only benchmark contract for review.  It intentionally serializes only
identifiers, hashes, paths, counters, provenance references, deterministic
rankings, and redacted diagnostics.  It never reads source payload files and
never carries article prose, table cells, captions, image bytes, embeddings,
vectors, model output, DSPy/RLM traces, optimizer state, graph writes, or import
eligibility claims.


Formerly: src/arxiv_archive/article_retrieval_tables.py"""

from __future__ import annotations

import json
import re
from copy import deepcopy
from dataclasses import dataclass
from typing import Any, Literal

ARTICLE_RETRIEVAL_TABLES_SCHEMA_VERSION = "article-retrieval-tables.v1"
ARTICLE_RETRIEVAL_TABLES_BUILDER = "metadata_only_article_retrieval_tables_v1"

BenchmarkStatus = Literal[
    "included_review_only",
    "blocked_review_only",
    "repair_required_review_only",
    "excluded_review_only",
]
DiagnosticSeverity = Literal["info", "warning", "repair_required", "error"]

ALLOWED_BENCHMARK_STATUSES = frozenset(BenchmarkStatus.__args__)  # type: ignore[attr-defined]
ALLOWED_SECTION_TYPES = frozenset(
    {
        "root",
        "abstract",
        "introduction",
        "background",
        "methods",
        "results",
        "discussion",
        "conclusion",
        "appendix",
        "unknown",
    }
)
DIAGNOSTIC_COUNTER_KEYS = (
    "duplicate_id_count",
    "malformed_source_ref_count",
    "missing_page_index_provenance_count",
    "missing_asset_provenance_count",
    "bad_vocabulary_count",
    "forbidden_payload_detection_count",
    "unsafe_authorization_count",
    "unsafe_readiness_count",
)

FORBIDDEN_PAYLOAD_KEYS = frozenset(
    {
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "claim_text",
        "section_text",
        "caption_text",
        "table_text",
        "equation_text",
        "model_output",
        "raw_model_output",
        "raw_minimax_response",
        "base64",
        "binary",
        "bytes",
        "image_bytes",
        "payload",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "secret",
        "secrets",
        "token",
        "tokens",
        "api_key",
        "credentials",
        "optimizer_trace",
        "optimizer_traces",
        "source_of_truth",
    }
)
UNSAFE_AUTHORIZATION_FLAGS = frozenset(
    {
        "trusted_kg_import_allowed",
        "ladybugdb_written",
        "production_import_attempted",
        "graph_import_claim",
        "import_eligible",
        "promoted_to_fact",
    }
)
UNSAFE_PAYLOAD_FLAGS = frozenset(
    {
        "raw_payloads_included",
        "raw_text_embedded",
        "raw_binary_embedded",
        "raw_table_embedded",
        "caption_embedded",
        "table_text_included",
        "caption_text_included",
        "embeddings_included",
        "embedding_included",
        "vectors_included",
        "vector_included",
        "optimizer_traces_included",
        "dspy_used",
        "rlm_used",
        "optimizer_used",
    }
)
UNSAFE_FALSE_FLAGS = UNSAFE_AUTHORIZATION_FLAGS | UNSAFE_PAYLOAD_FLAGS
UNSAFE_READINESS_STATUSES = frozenset(
    {"ready_for_import", "import_ready", "trusted", "promoted", "fact", "accepted_for_import"}
)
_SAFE_ID_RE = re.compile(r"^[A-Za-z0-9_.:/-]+$")
_SHA256_RE = re.compile(r"^[a-fA-F0-9]{64}$")


@dataclass(frozen=True)
class ArticleRetrievalTableDiagnostic:
    """One stable, redacted diagnostic for retrieval/table benchmark manifests."""

    code: str
    json_path: str
    severity: DiagnosticSeverity = "repair_required"
    object_id: str | None = None
    message: str = "Article retrieval/table benchmark diagnostic; inspect stable code and JSON path, not source content."
    blocks_import: bool = True

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "json_path": self.json_path,
            "severity": self.severity,
            "object_id": self.object_id,
            "message": self.message,
            "blocks_import": self.blocks_import,
        }


def _diagnostic(
    code: str,
    json_path: str,
    *,
    severity: DiagnosticSeverity = "repair_required",
    object_id: str | None = None,
    blocks_import: bool = True,
) -> ArticleRetrievalTableDiagnostic:
    return ArticleRetrievalTableDiagnostic(
        code=code,
        json_path=json_path,
        severity=severity,
        object_id=object_id,
        blocks_import=blocks_import,
    )


def _as_mapping(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _as_list(value: Any) -> list[Any]:
    return value if isinstance(value, list) else []


def _safe_str(value: Any) -> str:
    return value if isinstance(value, str) else ""


def _stable_unique(values: list[Any]) -> list[str]:
    seen: set[str] = set()
    unique: list[str] = []
    for value in values:
        if isinstance(value, str) and value not in seen:
            seen.add(value)
            unique.append(value)
    return unique


def _ref_ids(refs: Any, key: str) -> set[str]:
    if isinstance(refs, dict):
        values = refs.get(key)
        return {value for value in _as_list(values) if isinstance(value, str)}
    return set()


def _source_ref_ids(source_refs: Any) -> set[str]:
    return {
        source.get("source_id")
        for source in _as_list(source_refs)
        if isinstance(source, dict) and isinstance(source.get("source_id"), str)
    }


def _json_child_path(parent: str, child: str | int) -> str:
    if isinstance(child, int):
        return f"{parent}[{child}]"
    return f"{parent}.{child}" if parent != "$" else f"$.{child}"


def _iter_payload_paths(value: Any, path: str = "$") -> list[tuple[str, str, Any]]:
    findings: list[tuple[str, str, Any]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = _json_child_path(path, str(key))
            if str(key) in FORBIDDEN_PAYLOAD_KEYS:
                findings.append((str(key), child_path, child))
            findings.extend(_iter_payload_paths(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            findings.extend(_iter_payload_paths(child, _json_child_path(path, index)))
    return findings


def _iter_unsafe_true_paths(value: Any, path: str = "$") -> list[tuple[str, str]]:
    findings: list[tuple[str, str]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = _json_child_path(path, str(key))
            if str(key) in UNSAFE_FALSE_FLAGS and child is True:
                findings.append((str(key), child_path))
            findings.extend(_iter_unsafe_true_paths(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            findings.extend(_iter_unsafe_true_paths(child, _json_child_path(path, index)))
    return findings


def _iter_readiness_paths(value: Any, path: str = "$") -> list[str]:
    findings: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = _json_child_path(path, str(key))
            if str(key) in {"status", "benchmark_status", "review_state"} and isinstance(
                child, str
            ):
                if child in UNSAFE_READINESS_STATUSES:
                    findings.append(child_path)
            findings.extend(_iter_readiness_paths(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            findings.extend(_iter_readiness_paths(child, _json_child_path(path, index)))
    return findings


def _redact(value: Any) -> Any:
    if isinstance(value, dict):
        redacted: dict[str, Any] = {}
        for key in sorted(value):
            if key in FORBIDDEN_PAYLOAD_KEYS:
                continue
            child = value[key]
            if key in UNSAFE_FALSE_FLAGS:
                redacted[key] = False
            elif key == "metadata_only":
                redacted[key] = True
            elif key == "cpu_only":
                redacted[key] = True
            elif key == "review_only":
                redacted[key] = True
            elif key == "status" and child in UNSAFE_READINESS_STATUSES:
                redacted[key] = "review_only_not_import_eligible"
            elif key == "benchmark_status" and child not in ALLOWED_BENCHMARK_STATUSES:
                redacted[key] = "blocked_review_only"
            else:
                redacted[key] = _redact(child)
        return redacted
    if isinstance(value, list):
        return [_redact(child) for child in value]
    return value


def default_safety_flags() -> dict[str, bool]:
    """Return the required safe flags for this metadata-only boundary."""

    return {
        "metadata_only": True,
        "cpu_only": True,
        "review_only": True,
        "raw_payloads_included": False,
        "table_text_included": False,
        "caption_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "dspy_used": False,
        "rlm_used": False,
        "optimizer_used": False,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def default_bridge_subtree() -> dict[str, Any]:
    """Return the fixed review-only bridge subtree attached to manifests."""

    return {
        "status": "review_only_not_import_eligible",
        "source_slice": "M024-0xjwh9/S06",
        "graph_import_claim": False,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "raw_payloads_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "optimizer_traces_included": False,
    }


def _ranking_tie_count(records: list[dict[str, Any]]) -> int:
    score_counts: dict[float, int] = {}
    for record in records:
        score = record.get("benchmark_score", 0)
        try:
            normalized = float(score)
        except (TypeError, ValueError):
            normalized = 0.0
        score_counts[normalized] = score_counts.get(normalized, 0) + 1
    return sum(count - 1 for count in score_counts.values() if count > 1)


def _rank_records(records: list[dict[str, Any]], id_key: str) -> list[dict[str, Any]]:
    ranked = [deepcopy(record) for record in records if isinstance(record, dict)]

    def sort_key(record: dict[str, Any]) -> tuple[float, str]:
        try:
            score = float(record.get("benchmark_score", 0.0))
        except (TypeError, ValueError):
            score = 0.0
        return (-score, _safe_str(record.get(id_key)))

    ranked.sort(key=sort_key)
    for index, record in enumerate(ranked, start=1):
        record["rank"] = index
        record.setdefault("benchmark_status", "included_review_only")
        record.setdefault("diagnostic_codes", [])
        record["embedding_included"] = False
        record["vector_included"] = False
        record["import_eligible"] = False
        record["promoted_to_fact"] = False
        if id_key == "unit_id":
            record["raw_text_embedded"] = False
        else:
            record["raw_table_embedded"] = False
            record["caption_embedded"] = False
    return ranked


def _manifest_provenance_count(page_index_refs: Any, asset_refs: Any, links_dedup_refs: Any) -> int:
    count = 0
    for refs in (page_index_refs, asset_refs, links_dedup_refs):
        mapping = _as_mapping(refs)
        if (
            mapping.get("manifest_path")
            or mapping.get("manifest_sha256")
            or mapping.get("schema_version")
        ):
            count += 1
    return count


def summarize_article_retrieval_tables(manifest: dict[str, Any]) -> dict[str, Any]:
    """Return aggregate benchmark counters with import/readiness values clamped to zero."""

    manifest = _as_mapping(manifest)
    retrieval_units = [
        unit for unit in _as_list(manifest.get("retrieval_units")) if isinstance(unit, dict)
    ]
    table_candidates = [
        candidate
        for candidate in _as_list(manifest.get("table_candidates"))
        if isinstance(candidate, dict)
    ]
    all_records = retrieval_units + table_candidates
    diagnostics = validate_article_retrieval_table_manifest(manifest)
    diagnostic_counts = dict.fromkeys(DIAGNOSTIC_COUNTER_KEYS, 0)
    code_to_counter = {
        "duplicate_id": "duplicate_id_count",
        "malformed_source_ref": "malformed_source_ref_count",
        "missing_page_index_provenance": "missing_page_index_provenance_count",
        "missing_asset_provenance": "missing_asset_provenance_count",
        "bad_vocabulary": "bad_vocabulary_count",
        "forbidden_payload_key": "forbidden_payload_detection_count",
        "unsafe_authorization": "unsafe_authorization_count",
        "unsafe_readiness": "unsafe_readiness_count",
    }
    for diagnostic in diagnostics:
        counter_key = code_to_counter.get(diagnostic.code)
        if counter_key is not None:
            diagnostic_counts[counter_key] += 1

    page_index_refs = _as_mapping(manifest.get("page_index_refs"))
    asset_refs = _as_mapping(manifest.get("asset_refs"))
    links_dedup_refs = _as_mapping(manifest.get("links_dedup_refs"))
    link_ids = set(_as_list(links_dedup_refs.get("metadata_signal_ids"))) | set(
        _as_list(links_dedup_refs.get("dedup_candidate_ids"))
    )
    return {
        "retrieval_unit_count": len(retrieval_units),
        "table_candidate_count": len(table_candidates),
        "included_review_only_count": sum(
            1 for record in all_records if record.get("benchmark_status") == "included_review_only"
        ),
        "blocked_count": sum(
            1 for record in all_records if record.get("benchmark_status") == "blocked_review_only"
        ),
        "repair_required_count": sum(
            1
            for record in all_records
            if record.get("benchmark_status") == "repair_required_review_only"
        ),
        "source_ref_count": len(_source_ref_ids(manifest.get("source_refs"))),
        "page_index_node_ref_count": len(_ref_ids(page_index_refs, "node_ids")),
        "page_index_anchor_ref_count": len(_ref_ids(page_index_refs, "anchor_ids")),
        "asset_ref_count": len(_ref_ids(asset_refs, "asset_ids")),
        "link_provenance_ref_count": len({value for value in link_ids if isinstance(value, str)}),
        "manifest_provenance_count": _manifest_provenance_count(
            page_index_refs, asset_refs, links_dedup_refs
        ),
        "ranking_tie_count": _ranking_tie_count(retrieval_units),
        "diagnostic_counts": diagnostic_counts,
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "ladybugdb_written_count": 0,
        "production_import_attempted_count": 0,
        "graph_readiness_count": 0,
    }


def build_article_retrieval_table_manifest(
    *,
    paper_id: str,
    run_id: str,
    source_refs: list[dict[str, Any]],
    page_index_refs: dict[str, Any],
    asset_refs: dict[str, Any],
    links_dedup_refs: dict[str, Any],
    retrieval_units: list[dict[str, Any]],
    table_candidates: list[dict[str, Any]],
    manifest_path: str,
) -> dict[str, Any]:
    """Build a deterministic metadata-only retrieval/table benchmark manifest.

    All inputs are treated as untrusted JSON-like values.  The builder ranks by
    ``benchmark_score`` descending and stable IDs ascending, clamps unsafe flags
    to false, and attaches summary/bridge/safety sections without graph writes.
    """

    safe_retrieval_units = _rank_records(_as_list(retrieval_units), "unit_id")
    safe_table_candidates = _rank_records(_as_list(table_candidates), "candidate_id")
    ranking_diagnostics: list[dict[str, Any]] = []
    if _ranking_tie_count(safe_retrieval_units):
        ranking_diagnostics.append(
            {
                "code": "stable_tiebreaker_unit_id_applied",
                "json_path": "$.retrieval_units",
                "severity": "info",
                "message": "Equal benchmark scores are ordered by stable unit_id without reading payload text.",
                "blocks_import": False,
            }
        )

    manifest: dict[str, Any] = {
        "schema_version": ARTICLE_RETRIEVAL_TABLES_SCHEMA_VERSION,
        "paper_id": paper_id,
        "run_id": run_id,
        "manifest_path": manifest_path,
        "manifest_sha256": "0" * 64,
        "manifest_schema": ARTICLE_RETRIEVAL_TABLES_SCHEMA_VERSION,
        "builder": ARTICLE_RETRIEVAL_TABLES_BUILDER,
        "source_refs": deepcopy(source_refs),
        "page_index_refs": deepcopy(page_index_refs),
        "asset_refs": deepcopy(asset_refs),
        "links_dedup_refs": deepcopy(links_dedup_refs),
        "retrieval_units": safe_retrieval_units,
        "table_candidates": safe_table_candidates,
        "ranking_diagnostics": ranking_diagnostics,
        "bridge_subtree": default_bridge_subtree(),
        "safety_flags": default_safety_flags(),
        "diagnostics": [],
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
    }
    manifest["summary"] = summarize_article_retrieval_tables(manifest)
    manifest["diagnostics"] = [
        diagnostic.to_redacted_dict()
        for diagnostic in validate_article_retrieval_table_manifest(manifest)
    ]
    return to_redacted_dict(manifest)


def validate_article_retrieval_table_manifest(
    manifest: dict[str, Any],
) -> list[ArticleRetrievalTableDiagnostic]:
    """Validate a metadata-only manifest and return stable redacted diagnostics."""

    manifest = _as_mapping(manifest)
    diagnostics: list[ArticleRetrievalTableDiagnostic] = []

    if manifest.get("schema_version") != ARTICLE_RETRIEVAL_TABLES_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("bad_vocabulary", "$.schema_version"))
    manifest_sha = manifest.get("manifest_sha256")
    if manifest_sha is not None and (
        not isinstance(manifest_sha, str) or not _SHA256_RE.match(manifest_sha)
    ):
        diagnostics.append(_diagnostic("bad_vocabulary", "$.manifest_sha256"))

    source_ids = _source_ref_ids(manifest.get("source_refs"))
    for index, source in enumerate(_as_list(manifest.get("source_refs"))):
        if not isinstance(source, dict):
            diagnostics.append(_diagnostic("bad_vocabulary", f"$.source_refs[{index}]"))
            continue
        source_id = source.get("source_id")
        if not isinstance(source_id, str) or not _SAFE_ID_RE.match(source_id):
            diagnostics.append(
                _diagnostic("malformed_source_ref", f"$.source_refs[{index}].source_id")
            )

    page_index_refs = _as_mapping(manifest.get("page_index_refs"))
    page_index_node_ids = _ref_ids(page_index_refs, "node_ids")
    page_index_anchor_ids = _ref_ids(page_index_refs, "anchor_ids")
    asset_ids = _ref_ids(_as_mapping(manifest.get("asset_refs")), "asset_ids")

    seen_ids: set[str] = set()
    for collection_name, id_key in (
        ("retrieval_units", "unit_id"),
        ("table_candidates", "candidate_id"),
    ):
        for index, record in enumerate(_as_list(manifest.get(collection_name))):
            if not isinstance(record, dict):
                diagnostics.append(_diagnostic("bad_vocabulary", f"$.{collection_name}[{index}]"))
                continue
            record_id = record.get(id_key)
            if isinstance(record_id, str):
                if record_id in seen_ids:
                    diagnostics.append(
                        _diagnostic(
                            "duplicate_id",
                            f"$.{collection_name}[{index}].{id_key}",
                            object_id=record_id,
                        )
                    )
                seen_ids.add(record_id)
            else:
                diagnostics.append(
                    _diagnostic("bad_vocabulary", f"$.{collection_name}[{index}].{id_key}")
                )

            status = record.get("benchmark_status")
            if status not in ALLOWED_BENCHMARK_STATUSES:
                diagnostics.append(
                    _diagnostic("bad_vocabulary", f"$.{collection_name}[{index}].benchmark_status")
                )

            section_type = record.get("section_type")
            if section_type is not None and section_type not in ALLOWED_SECTION_TYPES:
                diagnostics.append(
                    _diagnostic("bad_vocabulary", f"$.{collection_name}[{index}].section_type")
                )

            node_id = record.get("page_index_node_id")
            if isinstance(node_id, str) and node_id not in page_index_node_ids:
                diagnostics.append(
                    _diagnostic(
                        "missing_page_index_provenance",
                        f"$.{collection_name}[{index}].page_index_node_id",
                    )
                )
            anchor_id = record.get("page_index_anchor_id")
            if isinstance(anchor_id, str) and anchor_id not in page_index_anchor_ids:
                diagnostics.append(
                    _diagnostic(
                        "missing_page_index_provenance",
                        f"$.{collection_name}[{index}].page_index_anchor_id",
                    )
                )
            for ref_index, source_ref_id in enumerate(_as_list(record.get("source_ref_ids"))):
                if not isinstance(source_ref_id, str) or source_ref_id not in source_ids:
                    diagnostics.append(
                        _diagnostic(
                            "malformed_source_ref",
                            f"$.{collection_name}[{index}].source_ref_ids[{ref_index}]",
                        )
                    )

            if collection_name == "table_candidates":
                asset_id = record.get("asset_id")
                if isinstance(asset_id, str) and asset_id not in asset_ids:
                    diagnostics.append(
                        _diagnostic(
                            "missing_asset_provenance", f"$.table_candidates[{index}].asset_id"
                        )
                    )
                transformation_plan = _as_mapping(record.get("transformation_plan"))
                for flag_name in ("raw_table_cells_included", "caption_included"):
                    if transformation_plan.get(flag_name) is True:
                        diagnostics.append(
                            _diagnostic(
                                "forbidden_payload_key",
                                f"$.table_candidates[{index}].transformation_plan.{flag_name}",
                            )
                        )

    for _key, path, _value in _iter_payload_paths(manifest):
        diagnostics.append(_diagnostic("forbidden_payload_key", path))

    for key, path in _iter_unsafe_true_paths(manifest):
        code = (
            "unsafe_authorization" if key in UNSAFE_AUTHORIZATION_FLAGS else "forbidden_payload_key"
        )
        diagnostics.append(_diagnostic(code, path))

    for path in _iter_readiness_paths(manifest):
        diagnostics.append(_diagnostic("unsafe_readiness", path))

    # Preserve deterministic order while avoiding duplicate code/path pairs when a
    # single unsafe field is caught by more than one structural rule.
    unique: list[ArticleRetrievalTableDiagnostic] = []
    seen_pairs: set[tuple[str, str]] = set()
    for diagnostic in diagnostics:
        pair = (diagnostic.code, diagnostic.json_path)
        if pair in seen_pairs:
            continue
        seen_pairs.add(pair)
        unique.append(diagnostic)
    return unique


def to_redacted_dict(manifest: dict[str, Any]) -> dict[str, Any]:
    """Return a deterministic JSON-native manifest with forbidden payloads removed."""

    redacted = _redact(_as_mapping(manifest))
    if not isinstance(redacted, dict):
        return {}
    if "safety_flags" in redacted:
        redacted["safety_flags"] = {
            **default_safety_flags(),
            **_as_mapping(redacted.get("safety_flags")),
        }
        for key, value in default_safety_flags().items():
            redacted["safety_flags"][key] = value
    if "bridge_subtree" in redacted:
        redacted["bridge_subtree"] = {
            **default_bridge_subtree(),
            **_as_mapping(redacted.get("bridge_subtree")),
        }
        for key, value in default_bridge_subtree().items():
            redacted["bridge_subtree"][key] = value
    if "summary" in redacted:
        redacted["summary"] = {**_as_mapping(redacted.get("summary"))}
        for key in (
            "import_eligible_count",
            "promoted_to_fact_count",
            "ladybugdb_written_count",
            "production_import_attempted_count",
            "graph_readiness_count",
        ):
            redacted["summary"][key] = 0
    redacted["import_eligible_count"] = 0
    redacted["promoted_to_fact_count"] = 0
    return redacted


def to_json(manifest: dict[str, Any]) -> str:
    """Serialize a redacted manifest as deterministic pretty JSON."""

    return json.dumps(to_redacted_dict(manifest), sort_keys=True, indent=2) + "\n"


__all__ = [
    "ARTICLE_RETRIEVAL_TABLES_SCHEMA_VERSION",
    "ALLOWED_BENCHMARK_STATUSES",
    "DIAGNOSTIC_COUNTER_KEYS",
    "ArticleRetrievalTableDiagnostic",
    "build_article_retrieval_table_manifest",
    "default_bridge_subtree",
    "default_safety_flags",
    "summarize_article_retrieval_tables",
    "to_json",
    "to_redacted_dict",
    "validate_article_retrieval_table_manifest",
]
