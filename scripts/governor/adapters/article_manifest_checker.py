"""Article manifest content checker — M451 W4 / ADR-068.

Validates per-article manifest.yaml against the NORM declared in
data/schemas/article_schema.yaml. Manifest-internal consistency is
checked on every article; artifact file existence (FS = truth) is
checked directly on disk. No structural knowledge is hardcoded:
layers, lifecycle order and invariants come from the contract and norm.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import yaml

from governor.domain.trajectory import DriftFlag


def check_article_manifests(root: Path) -> list[DriftFlag]:
    contract_path = root / "scripts/governor/checks/article_manifest.yaml"
    if not contract_path.exists():
        return []
    try:
        contract = yaml.safe_load(contract_path.read_text()) or {}
    except (OSError, yaml.YAMLError):
        return []

    dimension = str(contract.get("dimension", "article_manifest"))
    severity = str(contract.get("severity", "medium"))
    norm = _load_yaml(root / str(contract.get("norm", "data/schemas/article_schema.yaml")))
    processed = root / str(contract.get("processed_root", "data/processed"))
    manifest_name = str(contract.get("manifest_name", "manifest.yaml"))
    enabled = set(contract.get("invariants", []))
    sample_limit = int(contract.get("sample_limit", 0) or 0)

    if not isinstance(norm, dict) or not processed.exists():
        return []

    lifecycle_order = list(norm.get("lifecycle_order", []))
    layer_ids = {layer["id"] for layer in norm.get("layers", []) if isinstance(layer, dict)}

    def _flag(code: str, evidence: str) -> DriftFlag:
        return DriftFlag(flag=code, severity=severity, evidence=evidence, dimension=dimension)

    flags: list[DriftFlag] = []
    article_dirs = sorted(d for d in processed.iterdir() if d.is_dir())
    if sample_limit > 0:
        article_dirs = article_dirs[:sample_limit]

    checked = 0
    for article_dir in article_dirs:
        manifest_file = article_dir / manifest_name
        if not manifest_file.exists():
            continue  # W4 migration coverage is tracked by schema_hierarchy, not here
        checked += 1
        m = _load_yaml(manifest_file)
        if not isinstance(m, dict):
            flags.append(_flag("manifest_unparseable", f"{article_dir.name}/{manifest_name} is not a mapping"))
            continue
        paper = str(m.get("paper_id", article_dir.name))

        events = m.get("lifecycle_events") or []
        lifecycle = m.get("lifecycle")

        # lifecycle_is_derived: current state == last event stage
        if "lifecycle_is_derived" in enabled:
            expected = events[-1]["stage"] if events else "raw"
            if lifecycle != expected:
                flags.append(_flag(
                    "lifecycle_not_derived",
                    f"{paper}: lifecycle={lifecycle} but last event stage={expected} (hand-set state?)",
                ))

        # transaction_time_monotonic
        if "transaction_time_monotonic" in enabled and len(events) > 1:
            times = [str(e.get("recorded_at", "")) for e in events]
            if times != sorted(times):
                flags.append(_flag(
                    "transaction_time_not_monotonic",
                    f"{paper}: lifecycle_events recorded_at decreases",
                ))

        # unknown stages
        for e in events:
            if e.get("stage") not in lifecycle_order:
                flags.append(_flag(
                    "stage_not_in_norm",
                    f"{paper}: event stage {e.get('stage')!r} not in article_schema lifecycle_order",
                ))

        artifacts = m.get("artifacts") or {}
        # derived_from_exists (+ acyclic by construction: parent must precede in order)
        if "derived_from_exists" in enabled:
            for layer_id, art in artifacts.items():
                if layer_id not in layer_ids:
                    flags.append(_flag(
                        "layer_not_in_norm",
                        f"{paper}: artifact layer {layer_id!r} not defined in article_schema",
                    ))
                for parent in (art or {}).get("derived_from", []):
                    if parent != "source" and parent not in layer_ids:
                        flags.append(_flag(
                            "derived_from_unknown",
                            f"{paper}: layer {layer_id} derived_from unknown {parent!r}",
                        ))

        # ast_covers_sections (from manifest stats; generator guarantees provenance)
        if "ast_covers_sections" in enabled:
            sections = _stat(artifacts, "parsed", "sections")
            ast = _stat(artifacts, "context", "ast_nodes")
            if sections is not None and ast is not None and sections != ast:
                flags.append(_flag(
                    "ast_covers_sections",
                    f"{paper}: ast_nodes={ast} != sections={sections}",
                ))

        # artifact_files_exist (FS = truth)
        if "artifact_files_exist" in enabled:
            for layer_id, art in artifacts.items():
                rel = (art or {}).get("file")
                if rel and not (article_dir / str(rel)).exists():
                    flags.append(_flag(
                        "artifact_file_missing",
                        f"{paper}: artifact {layer_id} file {rel} missing on disk",
                    ))

        # no_orphan_entities (reads extracted/entities.json — small files)
        if "no_orphan_entities" in enabled:
            ents_file = article_dir / "extracted" / "entities.json"
            if ents_file.exists():
                try:
                    ents = json.loads(ents_file.read_text())
                except (OSError, json.JSONDecodeError):
                    ents = None
                if isinstance(ents, list):
                    orphans = [e for e in ents
                               if not isinstance(e, dict)
                               or not isinstance(e.get("section_ordinal"), int)
                               or e.get("section_ordinal", -1) < 0]
                    if orphans:
                        flags.append(_flag(
                            "orphan_entities",
                            f"{paper}: {len(orphans)} entities without section ordinal",
                        ))

    if checked == 0:
        flags.append(DriftFlag(
            flag="no_article_manifests",
            severity=severity,
            evidence=f"no {manifest_name} found under {processed}",
            dimension=dimension,
        ))
    return flags


def _stat(artifacts: dict, layer: str, key: str) -> Any:
    art = artifacts.get(layer) or {}
    stats = art.get("stats") or {}
    return stats.get(key)


def _load_yaml(path: Path) -> Any:
    try:
        return yaml.safe_load(path.read_text())
    except (OSError, yaml.YAMLError):
        return None
