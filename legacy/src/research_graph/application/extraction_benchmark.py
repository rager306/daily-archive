# Formerly: src/arxiv_archive/extraction_benchmark.py

"""Deterministic extraction benchmark metrics for future DSPy/MiniMax work.

The evaluator operates on metadata-only fixture files. It never calls external
models, never reads raw article text, and never authorizes graph writes or fact
promotion.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

METADATA_REF_PATTERN = re.compile(r"^[a-z][a-z0-9_.-]*:[A-Za-z0-9][A-Za-z0-9_.:/@-]*$")


@dataclass(frozen=True)
class CountMetrics:
    true_positive: int
    predicted: int
    gold: int

    @property
    def precision(self) -> float:
        if self.predicted == 0:
            return 1.0 if self.gold == 0 else 0.0
        return self.true_positive / self.predicted

    @property
    def recall(self) -> float:
        if self.gold == 0:
            return 1.0 if self.predicted == 0 else 0.0
        return self.true_positive / self.gold

    @property
    def f1(self) -> float:
        precision = self.precision
        recall = self.recall
        if precision + recall == 0:
            return 0.0
        return 2 * precision * recall / (precision + recall)


def load_jsonl(path: str | Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    with Path(path).open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            stripped = line.strip()
            if not stripped:
                continue
            try:
                record = json.loads(stripped)
            except json.JSONDecodeError as exc:
                raise ValueError(f"{path}:{line_number}: invalid JSON") from exc
            if not isinstance(record, dict):
                raise ValueError(f"{path}:{line_number}: record must be an object")
            records.append(record)
    return records


def evaluate_files(gold_path: str | Path, prediction_path: str | Path) -> dict[str, Any]:
    return evaluate_records(load_jsonl(gold_path), load_jsonl(prediction_path))


def evaluate_records(
    gold_records: list[dict[str, Any]], prediction_records: list[dict[str, Any]]
) -> dict[str, Any]:
    gold_by_case = _records_by_case(gold_records, "gold")
    prediction_by_case = _records_by_case(prediction_records, "prediction")
    missing_predictions = sorted(set(gold_by_case) - set(prediction_by_case))
    extra_predictions = sorted(set(prediction_by_case) - set(gold_by_case))
    shared_case_ids = sorted(set(gold_by_case) & set(prediction_by_case))

    gold_entity_keys: set[tuple[str, str, str]] = set()
    prediction_entity_keys: set[tuple[str, str, str]] = set()
    gold_relation_keys: set[tuple[str, str, tuple[str, str], tuple[str, str]]] = set()
    prediction_relation_keys: set[tuple[str, str, tuple[str, str], tuple[str, str]]] = set()

    predicted_items = 0
    predicted_items_with_valid_evidence = 0
    schema_valid_count = 0
    json_valid_count = 0
    total_cost = 0.0
    total_latency = 0.0
    total_retry_count = 0
    prediction_count = len(prediction_records)
    invalid_cases: list[str] = []

    for case_id in shared_case_ids:
        gold_record = gold_by_case[case_id]
        prediction_record = prediction_by_case[case_id]
        _validate_record(gold_record, role="gold")
        schema_errors = _validate_record(prediction_record, role="prediction")
        if schema_errors:
            invalid_cases.append(case_id)
        if bool(prediction_record.get("schema_valid")) and not schema_errors:
            schema_valid_count += 1
        if bool(prediction_record.get("json_valid")):
            json_valid_count += 1

        gold_entity_map = _entity_key_by_id(gold_record)
        prediction_entity_map = _entity_key_by_id(prediction_record)

        gold_entity_keys.update((case_id, *entity_key) for entity_key in gold_entity_map.values())
        prediction_entity_keys.update(
            (case_id, *entity_key) for entity_key in prediction_entity_map.values()
        )
        gold_relation_keys.update(_relation_keys(case_id, gold_record, gold_entity_map))
        prediction_relation_keys.update(
            _relation_keys(case_id, prediction_record, prediction_entity_map)
        )

        item_count, valid_item_count = _evidence_counts(prediction_record)
        predicted_items += item_count
        predicted_items_with_valid_evidence += valid_item_count

        operational = prediction_record.get("operational", {})
        total_cost += float(operational.get("cost_estimate", 0.0))
        total_latency += float(operational.get("latency_ms", 0.0))
        total_retry_count += int(operational.get("retry_count", 0))

    for case_id in set(gold_by_case) - set(prediction_by_case):
        gold_record = gold_by_case[case_id]
        _validate_record(gold_record, role="gold")
        gold_entity_map = _entity_key_by_id(gold_record)
        gold_entity_keys.update((case_id, *entity_key) for entity_key in gold_entity_map.values())
        gold_relation_keys.update(_relation_keys(case_id, gold_record, gold_entity_map))

    entity_counts = CountMetrics(
        true_positive=len(gold_entity_keys & prediction_entity_keys),
        predicted=len(prediction_entity_keys),
        gold=len(gold_entity_keys),
    )
    relation_counts = CountMetrics(
        true_positive=len(gold_relation_keys & prediction_relation_keys),
        predicted=len(prediction_relation_keys),
        gold=len(gold_relation_keys),
    )

    evidence_path_validity = (
        predicted_items_with_valid_evidence / predicted_items if predicted_items else 1.0
    )
    denominator = prediction_count or 1

    return {
        "case_count": len(gold_records),
        "prediction_count": prediction_count,
        "missing_predictions": missing_predictions,
        "extra_predictions": extra_predictions,
        "invalid_cases": invalid_cases,
        "entity_true_positive": entity_counts.true_positive,
        "entity_predicted": entity_counts.predicted,
        "entity_gold": entity_counts.gold,
        "entity_precision": entity_counts.precision,
        "entity_recall": entity_counts.recall,
        "entity_f1": entity_counts.f1,
        "relation_true_positive": relation_counts.true_positive,
        "relation_predicted": relation_counts.predicted,
        "relation_gold": relation_counts.gold,
        "relation_precision": relation_counts.precision,
        "relation_recall": relation_counts.recall,
        "relation_f1": relation_counts.f1,
        "evidence_path_validity": evidence_path_validity,
        "schema_validity": schema_valid_count / denominator,
        "json_validity": json_valid_count / denominator,
        "mean_cost_estimate": total_cost / denominator,
        "mean_latency_ms": total_latency / denominator,
        "total_retry_count": total_retry_count,
    }


def _records_by_case(records: list[dict[str, Any]], role: str) -> dict[str, dict[str, Any]]:
    by_case: dict[str, dict[str, Any]] = {}
    for record in records:
        case_id = record.get("case_id")
        if not isinstance(case_id, str) or not case_id:
            raise ValueError(f"{role} record missing case_id")
        if case_id in by_case:
            raise ValueError(f"duplicate {role} case_id: {case_id}")
        by_case[case_id] = record
    return by_case


def _validate_record(record: dict[str, Any], *, role: str) -> list[str]:
    errors: list[str] = []
    for key in (
        "case_id",
        "paper_id",
        "source_artifact_refs",
        "entities",
        "relations",
        "schema_valid",
        "json_valid",
        "operational",
    ):
        if key not in record:
            errors.append(f"missing:{key}")
    if not isinstance(record.get("source_artifact_refs"), list) or not all(
        _is_metadata_ref(ref) for ref in record.get("source_artifact_refs", [])
    ):
        errors.append("invalid:source_artifact_refs")
    if not isinstance(record.get("schema_valid"), bool):
        errors.append("invalid:schema_valid")
    if not isinstance(record.get("json_valid"), bool):
        errors.append("invalid:json_valid")
    if not isinstance(record.get("entities"), list):
        errors.append("invalid:entities")
    else:
        for entity in record["entities"]:
            errors.extend(_validate_entity(entity))
    if not isinstance(record.get("relations"), list):
        errors.append("invalid:relations")
    else:
        entity_ids = {
            entity.get("id") for entity in record.get("entities", []) if isinstance(entity, dict)
        }
        for relation in record["relations"]:
            errors.extend(_validate_relation(relation, entity_ids))
    operational = record.get("operational")
    if not isinstance(operational, dict):
        errors.append("invalid:operational")
    else:
        for key in ("cost_estimate", "latency_ms", "retry_count"):
            if key not in operational:
                errors.append(f"missing:operational.{key}")
        if _negative_number(operational.get("cost_estimate")):
            errors.append("invalid:operational.cost_estimate")
        if _negative_number(operational.get("latency_ms")):
            errors.append("invalid:operational.latency_ms")
        if (
            not isinstance(operational.get("retry_count"), int)
            or operational.get("retry_count", 0) < 0
        ):
            errors.append("invalid:operational.retry_count")
    if role == "gold" and errors:
        raise ValueError(f"gold fixture is invalid for {record.get('case_id')}: {errors}")
    return errors


def _validate_entity(entity: Any) -> list[str]:
    if not isinstance(entity, dict):
        return ["invalid:entity"]
    errors: list[str] = []
    for key in ("id", "type", "label", "evidence_refs"):
        if key not in entity:
            errors.append(f"missing:entity.{key}")
    if not isinstance(entity.get("id"), str) or not entity.get("id"):
        errors.append("invalid:entity.id")
    if not isinstance(entity.get("type"), str) or not entity.get("type"):
        errors.append("invalid:entity.type")
    if not isinstance(entity.get("label"), str) or not entity.get("label"):
        errors.append("invalid:entity.label")
    if not isinstance(entity.get("evidence_refs"), list):
        errors.append("invalid:entity.evidence_refs")
    return errors


def _validate_relation(relation: Any, entity_ids: set[Any]) -> list[str]:
    if not isinstance(relation, dict):
        return ["invalid:relation"]
    errors: list[str] = []
    for key in ("id", "type", "source", "target", "evidence_refs"):
        if key not in relation:
            errors.append(f"missing:relation.{key}")
    if relation.get("source") not in entity_ids:
        errors.append("invalid:relation.source")
    if relation.get("target") not in entity_ids:
        errors.append("invalid:relation.target")
    if not isinstance(relation.get("evidence_refs"), list):
        errors.append("invalid:relation.evidence_refs")
    return errors


def _entity_key_by_id(record: dict[str, Any]) -> dict[str, tuple[str, str]]:
    result: dict[str, tuple[str, str]] = {}
    for entity in record.get("entities", []):
        if not isinstance(entity, dict):
            continue
        entity_id = entity.get("id")
        entity_type = entity.get("type")
        label = entity.get("label")
        if isinstance(entity_id, str) and isinstance(entity_type, str) and isinstance(label, str):
            result[entity_id] = (entity_type, _normalize_label(label))
    return result


def _relation_keys(
    case_id: str,
    record: dict[str, Any],
    entity_key_by_id: dict[str, tuple[str, str]],
) -> set[tuple[str, str, tuple[str, str], tuple[str, str]]]:
    keys: set[tuple[str, str, tuple[str, str], tuple[str, str]]] = set()
    for relation in record.get("relations", []):
        if not isinstance(relation, dict):
            continue
        relation_type = relation.get("type")
        source_key = entity_key_by_id.get(str(relation.get("source")))
        target_key = entity_key_by_id.get(str(relation.get("target")))
        if isinstance(relation_type, str) and source_key is not None and target_key is not None:
            keys.add((case_id, relation_type, source_key, target_key))
    return keys


def _evidence_counts(record: dict[str, Any]) -> tuple[int, int]:
    total = 0
    valid = 0
    for item in [*record.get("entities", []), *record.get("relations", [])]:
        if not isinstance(item, dict):
            continue
        total += 1
        refs = item.get("evidence_refs")
        if (
            isinstance(refs, list)
            and refs
            and all(isinstance(ref, str) and ref.startswith("evidence:") for ref in refs)
        ):
            valid += 1
    return total, valid


def _normalize_label(value: str) -> str:
    return " ".join(value.casefold().strip().split())


def _is_metadata_ref(value: Any) -> bool:
    return isinstance(value, str) and METADATA_REF_PATTERN.fullmatch(value) is not None


def _negative_number(value: Any) -> bool:
    return not isinstance(value, int | float) or isinstance(value, bool) or value < 0
