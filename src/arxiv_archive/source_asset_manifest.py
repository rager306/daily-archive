"""Redacted source artifact and multimodal asset manifest contract.

This module defines the executable subset of the M005/S05 source asset
preservation contract. It serializes paths, hashes, provenance, source spans,
and linkage metadata only. It must not serialize raw paper text, chunk text,
base64/binary payloads, embeddings, vectors, secrets, optimizer traces, or KG
facts.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from hashlib import sha256
from pathlib import Path
from shutil import copy2
from typing import Any, Literal

SCHEMA_VERSION = "m005-source-asset-manifest.v1"
TRUSTED_IMPORT_USE = "trusted_kg_import"

SourceRole = Literal["original_pdf", "normalized_markdown", "derived_asset"]
AssetType = Literal["source_pdf", "normalized_markdown", "figure", "table", "equation", "reference", "metadata"]
ExtractionState = Literal["preserved_source", "linked_not_extracted", "extracted", "missing_source"]

FORBIDDEN_RAW_FIELDS = frozenset(
    {
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "claim_text",
        "base64",
        "binary",
        "bytes",
        "image_bytes",
        "payload",
    }
)
FORBIDDEN_EMBEDDING_FIELDS = frozenset({"embedding", "embeddings"})
FORBIDDEN_VECTOR_FIELDS = frozenset({"vector", "vectors"})
FORBIDDEN_SECRET_FIELDS = frozenset({"secret", "secrets", "token", "tokens", "api_key", "credentials"})
FORBIDDEN_OPTIMIZER_FIELDS = frozenset({"optimizer_trace", "optimizer_traces"})


def _redaction_flags() -> dict[str, bool]:
    return {
        "raw_text_included": False,
        "chunk_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "optimizer_traces_included": False,
    }


@dataclass(frozen=True)
class SourceAssetRunResult:
    """Run-level result for deterministic source artifact preservation."""

    manifests: tuple[dict[str, Any], ...]
    summary: dict[str, Any]


@dataclass(frozen=True)
class SourceArtifactCandidate:
    """One candidate source path resolved from a gold-corpus paper entry."""

    source_role: SourceRole
    path: Path | None
    original_reference: str
    missing_code: str | None = None


@dataclass(frozen=True)
class AssetDiagnostic:
    """One redacted validation diagnostic for a source asset manifest."""

    reason: str
    object_id: str | None = None
    object_type: str | None = None
    blocks_import: bool = True


@dataclass(frozen=True)
class AssetValidationResult:
    """Validation result for a redacted source asset manifest."""

    valid_manifest: bool
    diagnostics: tuple[AssetDiagnostic, ...]

    @property
    def passed(self) -> bool:
        return self.valid_manifest and not self.diagnostics

    @property
    def refusal_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for diagnostic in self.diagnostics:
            counts[diagnostic.reason] = counts.get(diagnostic.reason, 0) + 1
        return dict(sorted(counts.items()))


@dataclass(frozen=True)
class SourceSpan:
    """Pointer into a source coordinate space without carrying source content."""

    coordinate_space: str
    char_start: int
    char_end: int
    page_start: int | None = None
    page_end: int | None = None
    bbox: tuple[float, float, float, float] | None = None

    def to_contract(self) -> dict[str, Any]:
        return {
            "coordinate_space": self.coordinate_space,
            "char_start": self.char_start,
            "char_end": self.char_end,
            "page_start": self.page_start,
            "page_end": self.page_end,
            "bbox": list(self.bbox) if self.bbox is not None else None,
        }


@dataclass(frozen=True)
class PreservedSourceFile:
    """A preserved source file represented only by path/hash metadata."""

    source_file_id: str
    paper_id: str
    source_role: SourceRole
    original_path: str
    workspace_path: str
    sha256: str
    byte_size: int
    media_type: str
    provenance: dict[str, Any] = field(default_factory=dict)
    copied: bool = True
    warnings: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        return {
            "source_file_id": self.source_file_id,
            "paper_id": self.paper_id,
            "source_role": self.source_role,
            "original_path": self.original_path,
            "workspace_path": self.workspace_path,
            "sha256": self.sha256,
            "byte_size": self.byte_size,
            "media_type": self.media_type,
            "provenance": dict(self.provenance),
            "copied": self.copied,
            "warnings": [
                {
                    "code": code,
                    "severity": "warning",
                    "message": "Source artifact warning; inspect code and provenance, not file contents.",
                    "object_id": self.source_file_id,
                    "blocks_import": True,
                }
                for code in self.warnings
            ],
            "redaction": _redaction_flags(),
            "production_import_attempted": False,
            "ladybugdb_written": False,
        }


@dataclass(frozen=True)
class AssetRecord:
    """A source or multimodal asset link that is not a KG fact."""

    asset_id: str
    paper_id: str
    asset_type: AssetType
    extraction_state: ExtractionState
    source_file_id: str | None = None
    chunk_id: str | None = None
    source_artifact: str | None = None
    source_span: SourceSpan | None = None
    workspace_path: str | None = None
    sha256: str | None = None
    byte_size: int | None = None
    media_type: str | None = None
    provenance: dict[str, Any] = field(default_factory=dict)
    warning_codes: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        return {
            "asset_id": self.asset_id,
            "paper_id": self.paper_id,
            "asset_type": self.asset_type,
            "extraction_state": self.extraction_state,
            "source_file_id": self.source_file_id,
            "chunk_id": self.chunk_id,
            "source_artifact": self.source_artifact,
            "source_span": self.source_span.to_contract() if self.source_span is not None else None,
            "workspace_path": self.workspace_path,
            "sha256": self.sha256,
            "byte_size": self.byte_size,
            "media_type": self.media_type,
            "provenance": dict(self.provenance),
            "warnings": [
                {
                    "code": code,
                    "severity": "repair_required",
                    "message": "Asset linkage requires review before use in extraction or retrieval.",
                    "object_id": self.asset_id,
                    "blocks_import": True,
                }
                for code in self.warning_codes
            ],
            "promoted_to_fact": False,
            "allowed_uses": ["source_review", "benchmark_diagnostics"],
            "excluded_uses": [TRUSTED_IMPORT_USE, "production_ladybugdb_write", "embedding_generation"],
            "redaction": _redaction_flags(),
            "production_import_attempted": False,
            "ladybugdb_written": False,
        }


@dataclass(frozen=True)
class SourceAssetManifest:
    """Per-paper redacted source artifact and asset linkage manifest."""

    paper_id: str
    workspace_root: str
    source_files: tuple[PreservedSourceFile, ...] = ()
    assets: tuple[AssetRecord, ...] = ()
    run_id: str = "m005-s05-source-assets"
    warnings: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        source_file_records = [source_file.to_contract() for source_file in self.source_files]
        asset_records = [asset.to_contract() for asset in self.assets]
        diagnostics = _diagnostics_for_manifest(
            source_files=source_file_records,
            assets=asset_records,
            manifest_warnings=self.warnings,
        )
        return {
            "schema_version": SCHEMA_VERSION,
            "paper_id": self.paper_id,
            "run_id": self.run_id,
            "workspace_root": self.workspace_root,
            "source_files": source_file_records,
            "assets": asset_records,
            "diagnostics": diagnostics,
            "warnings": [
                {
                    "code": code,
                    "severity": "warning",
                    "message": "Manifest warning; inspect source metadata and diagnostics, not raw content.",
                    "object_id": self.paper_id,
                    "blocks_import": True,
                }
                for code in self.warnings
            ],
            "promoted_to_fact_count": 0,
            "raw_text_included": False,
            "chunk_text_included": False,
            "raw_binary_included": False,
            "base64_included": False,
            "embeddings_included": False,
            "vectors_included": False,
            "secrets_included": False,
            "optimizer_traces_included": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
        }


def preserve_source_assets_for_paper(
    paper: dict[str, Any],
    *,
    workspace_root: Path,
    run_id: str = "m005-s05-source-assets",
) -> SourceAssetManifest:
    """Copy one paper's source PDF/Markdown artifacts into a deterministic workspace."""
    paper_id = str(paper["paper_id"])
    paper_workspace = Path(workspace_root) / "papers" / paper_id
    source_workspace = paper_workspace / "source"
    source_workspace.mkdir(parents=True, exist_ok=True)
    preserved: list[PreservedSourceFile] = []
    warnings: list[str] = []
    for candidate in _source_candidates_for_paper(paper):
        if candidate.path is None:
            if candidate.missing_code is not None:
                warnings.append(candidate.missing_code)
            continue
        stable_name = _stable_source_name(source_role=candidate.source_role, source_path=candidate.path)
        workspace_path = source_workspace / stable_name
        copy2(candidate.path, workspace_path)
        preserved.append(
            PreservedSourceFile(
                source_file_id=f"{paper_id}:source:{candidate.source_role}",
                paper_id=paper_id,
                source_role=candidate.source_role,
                original_path=str(candidate.path),
                workspace_path=str(workspace_path),
                sha256=_sha256_file(workspace_path),
                byte_size=workspace_path.stat().st_size,
                media_type=_media_type_for_path(workspace_path),
                provenance={
                    "source": "gold_manifest_required_path",
                    "original_reference": candidate.original_reference,
                    "preservation_method": "copy2",
                },
            )
        )
    return SourceAssetManifest(
        paper_id=paper_id,
        workspace_root=str(paper_workspace),
        source_files=tuple(preserved),
        assets=(),
        run_id=run_id,
        warnings=tuple(sorted(set(warnings))),
    )


def build_source_asset_run(
    manifest_path: Path,
    *,
    output_dir: Path,
    annotation_diagnostics_path: Path | None = None,
    structure_diagnostics_path: Path | None = None,
    run_id: str = "m005-s05-source-assets",
) -> SourceAssetRunResult:
    """Build a full source-asset run with optional annotation-derived asset links."""
    result = preserve_source_assets_manifest(manifest_path, output_dir=output_dir, run_id=run_id)
    manifests = result.manifests
    if annotation_diagnostics_path is not None:
        manifests = attach_annotation_asset_links(
            manifests,
            annotation_diagnostics_path=annotation_diagnostics_path,
            structure_diagnostics_path=structure_diagnostics_path,
        )
    return SourceAssetRunResult(manifests=manifests, summary=_summary_for_manifests(manifests, source_manifest=manifest_path))


def preserve_source_assets_manifest(
    manifest_path: Path,
    *,
    output_dir: Path,
    run_id: str = "m005-s05-source-assets",
) -> SourceAssetRunResult:
    """Preserve source artifacts for all papers in a gold-corpus manifest."""
    manifest = _load_json(manifest_path)
    output_dir = Path(output_dir)
    manifests = tuple(
        preserve_source_assets_for_paper(paper, workspace_root=output_dir, run_id=run_id).to_contract()
        for paper in manifest.get("papers", [])
        if isinstance(paper, dict)
    )
    return SourceAssetRunResult(manifests=manifests, summary=_summary_for_manifests(manifests, source_manifest=manifest_path))


def attach_annotation_asset_links(
    manifests: tuple[dict[str, Any], ...],
    *,
    annotation_diagnostics_path: Path,
    structure_diagnostics_path: Path | None = None,
) -> tuple[dict[str, Any], ...]:
    """Attach redacted asset-link records derived from S04 annotation diagnostics."""
    annotation_records = _load_jsonl(annotation_diagnostics_path)
    structure_spans = _chunk_source_spans_by_paper(structure_diagnostics_path) if structure_diagnostics_path is not None else {}
    assets_by_paper = _asset_records_from_annotation_diagnostics(
        annotation_records=annotation_records,
        structure_spans=structure_spans,
    )
    linked_manifests: list[dict[str, Any]] = []
    for manifest in manifests:
        paper_id = str(manifest["paper_id"])
        source_files = _list_of_dicts(manifest.get("source_files"))
        source_file_by_role = {str(source_file.get("source_role")): source_file for source_file in source_files}
        existing_assets = _list_of_dicts(manifest.get("assets"))
        generated_assets = [
            _asset_record_with_source_file(asset, source_file_by_role=source_file_by_role).to_contract()
            for asset in assets_by_paper.get(paper_id, ())
        ]
        updated = dict(manifest)
        updated["assets"] = existing_assets + generated_assets
        updated["diagnostics"] = _diagnostics_for_manifest(
            source_files=source_files,
            assets=updated["assets"],
            manifest_warnings=tuple(str(warning.get("code")) for warning in manifest.get("warnings", []) if isinstance(warning, dict)),
        )
        linked_manifests.append(updated)
    return tuple(linked_manifests)


def write_source_asset_run(result: SourceAssetRunResult, output_dir: Path) -> None:
    """Write redacted source preservation run artifacts."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "source-preservation-summary.json").write_text(
        _json_dumps(result.summary),
        encoding="utf-8",
    )
    (output_dir / "source-asset-summary.json").write_text(
        _json_dumps(result.summary),
        encoding="utf-8",
    )
    (output_dir / "source-asset-package-diagnostics.jsonl").write_text(
        "".join(json.dumps(_manifest_to_record(manifest), sort_keys=True) + "\n" for manifest in result.manifests),
        encoding="utf-8",
    )
    manifests_dir = output_dir / "manifests"
    manifests_dir.mkdir(parents=True, exist_ok=True)
    for manifest in result.manifests:
        (manifests_dir / f"{manifest['paper_id']}-source-assets.json").write_text(_json_dumps(manifest), encoding="utf-8")


def validate_source_asset_manifest(manifest: dict[str, Any]) -> AssetValidationResult:
    """Validate a redacted source asset manifest without reading referenced files."""
    diagnostics: list[AssetDiagnostic] = []
    diagnostics.extend(_validate_required_fields(manifest, fields=("schema_version", "paper_id", "source_files", "assets", "diagnostics"), object_id=None, object_type="manifest"))
    diagnostics.extend(_validate_redaction(manifest, object_id=_string_or_none(manifest.get("paper_id")), object_type="manifest"))
    if manifest.get("schema_version") != SCHEMA_VERSION:
        diagnostics.append(AssetDiagnostic(reason="schema_version_mismatch", object_type="manifest"))
    for field_name in (
        "raw_text_included",
        "chunk_text_included",
        "raw_binary_included",
        "base64_included",
        "embeddings_included",
        "vectors_included",
        "secrets_included",
        "optimizer_traces_included",
        "ladybugdb_written",
        "production_import_attempted",
    ):
        if manifest.get(field_name) is not False:
            diagnostics.append(AssetDiagnostic(reason=f"unsafe_{field_name}", object_id=_string_or_none(manifest.get("paper_id")), object_type="manifest"))
    source_files = _list_of_dicts(manifest.get("source_files"))
    assets = _list_of_dicts(manifest.get("assets"))
    source_file_ids = {_string_or_none(source_file.get("source_file_id")) for source_file in source_files}
    source_file_ids.discard(None)
    for source_file in source_files:
        diagnostics.extend(_validate_source_file(source_file, package_paper_id=_string_or_none(manifest.get("paper_id"))))
    for asset in assets:
        diagnostics.extend(
            _validate_asset_record(
                asset,
                package_paper_id=_string_or_none(manifest.get("paper_id")),
                source_file_ids=source_file_ids,
            )
        )
    diagnostics.extend(_validate_manifest_diagnostics(manifest.get("diagnostics"), source_files=source_files, assets=assets))
    return AssetValidationResult(valid_manifest=not diagnostics, diagnostics=tuple(diagnostics))


def _validate_source_file(source_file: dict[str, Any], *, package_paper_id: str | None) -> list[AssetDiagnostic]:
    source_file_id = _string_or_none(source_file.get("source_file_id"))
    diagnostics = _validate_required_fields(
        source_file,
        fields=("source_file_id", "paper_id", "source_role", "workspace_path", "sha256", "byte_size", "media_type", "provenance", "redaction"),
        object_id=source_file_id,
        object_type="source_file",
    )
    diagnostics.extend(_validate_redaction(source_file, object_id=source_file_id, object_type="source_file"))
    if _string_or_none(source_file.get("paper_id")) != package_paper_id:
        diagnostics.append(AssetDiagnostic(reason="paper_id_mismatch", object_id=source_file_id, object_type="source_file"))
    if not isinstance(source_file.get("byte_size"), int) or int(source_file.get("byte_size", 0)) < 0:
        diagnostics.append(AssetDiagnostic(reason="invalid_byte_size", object_id=source_file_id, object_type="source_file"))
    if source_file.get("sha256") is not None and not _valid_sha256(source_file.get("sha256")):
        diagnostics.append(AssetDiagnostic(reason="invalid_sha256", object_id=source_file_id, object_type="source_file"))
    return diagnostics


def _validate_asset_record(asset: dict[str, Any], *, package_paper_id: str | None, source_file_ids: set[str]) -> list[AssetDiagnostic]:
    asset_id = _string_or_none(asset.get("asset_id"))
    diagnostics = _validate_required_fields(
        asset,
        fields=("asset_id", "paper_id", "asset_type", "extraction_state", "promoted_to_fact", "allowed_uses", "excluded_uses", "redaction", "warnings"),
        object_id=asset_id,
        object_type="asset",
    )
    diagnostics.extend(_validate_redaction(asset, object_id=asset_id, object_type="asset"))
    if _string_or_none(asset.get("paper_id")) != package_paper_id:
        diagnostics.append(AssetDiagnostic(reason="paper_id_mismatch", object_id=asset_id, object_type="asset"))
    source_file_id = _string_or_none(asset.get("source_file_id"))
    if source_file_id is not None and source_file_id not in source_file_ids:
        diagnostics.append(AssetDiagnostic(reason="unresolved_source_file", object_id=asset_id, object_type="asset"))
    if asset.get("promoted_to_fact") is not False:
        diagnostics.append(AssetDiagnostic(reason="asset_promoted_to_fact", object_id=asset_id, object_type="asset"))
    if TRUSTED_IMPORT_USE in _string_list(asset.get("allowed_uses")):
        diagnostics.append(AssetDiagnostic(reason="asset_allows_trusted_import", object_id=asset_id, object_type="asset"))
    if TRUSTED_IMPORT_USE not in _string_list(asset.get("excluded_uses")):
        diagnostics.append(AssetDiagnostic(reason="asset_missing_import_exclusion", object_id=asset_id, object_type="asset"))
    if asset.get("source_span") is not None and not _valid_source_span(asset.get("source_span")):
        diagnostics.append(AssetDiagnostic(reason="invalid_source_span", object_id=asset_id, object_type="asset"))
    if asset.get("sha256") is not None and not _valid_sha256(asset.get("sha256")):
        diagnostics.append(AssetDiagnostic(reason="invalid_sha256", object_id=asset_id, object_type="asset"))
    return diagnostics


def _validate_manifest_diagnostics(value: Any, *, source_files: list[dict[str, Any]], assets: list[dict[str, Any]]) -> list[AssetDiagnostic]:
    if not isinstance(value, dict):
        return [AssetDiagnostic(reason="missing_diagnostics", object_type="diagnostics")]
    diagnostics = _validate_required_fields(
        value,
        fields=(
            "source_file_count",
            "asset_count",
            "hash_coverage_rate",
            "asset_counts_by_type",
            "extraction_state_counts",
            "raw_text_included",
            "chunk_text_included",
            "raw_binary_included",
            "base64_included",
            "embeddings_included",
            "vectors_included",
            "secrets_included",
            "ladybugdb_written",
            "production_import_attempted",
        ),
        object_id=None,
        object_type="diagnostics",
    )
    diagnostics.extend(_validate_redaction(value, object_id=None, object_type="diagnostics"))
    if value.get("source_file_count") != len(source_files):
        diagnostics.append(AssetDiagnostic(reason="source_file_count_mismatch", object_type="diagnostics"))
    if value.get("asset_count") != len(assets):
        diagnostics.append(AssetDiagnostic(reason="asset_count_mismatch", object_type="diagnostics"))
    for field_name in (
        "raw_text_included",
        "chunk_text_included",
        "raw_binary_included",
        "base64_included",
        "embeddings_included",
        "vectors_included",
        "secrets_included",
        "ladybugdb_written",
        "production_import_attempted",
    ):
        if value.get(field_name) is not False:
            diagnostics.append(AssetDiagnostic(reason=f"unsafe_{field_name}", object_type="diagnostics"))
    return diagnostics


def _diagnostics_for_manifest(
    *,
    source_files: list[dict[str, Any]],
    assets: list[dict[str, Any]],
    manifest_warnings: tuple[str, ...],
) -> dict[str, Any]:
    hashed_source_count = sum(1 for source_file in source_files if _valid_sha256(source_file.get("sha256")))
    return {
        "source_file_count": len(source_files),
        "asset_count": len(assets),
        "hash_coverage_rate": hashed_source_count / len(source_files) if source_files else 0.0,
        "asset_counts_by_type": _counts(asset.get("asset_type") for asset in assets),
        "extraction_state_counts": _counts(asset.get("extraction_state") for asset in assets),
        "warning_counts": _counts(manifest_warnings),
        "promoted_to_fact_count": sum(1 for asset in assets if asset.get("promoted_to_fact") is True),
        "raw_text_included": False,
        "chunk_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


ASSET_CHUNK_TYPES: dict[str, AssetType] = {
    "table_context": "table",
    "table_row_group": "table",
    "figure_caption_context": "figure",
    "equation_context": "equation",
    "reference_entry": "reference",
    "citation_context": "reference",
    "metadata": "metadata",
    "administrative": "metadata",
}


def _asset_records_from_annotation_diagnostics(
    *,
    annotation_records: list[dict[str, Any]],
    structure_spans: dict[str, dict[str, dict[str, Any]]],
) -> dict[str, tuple[AssetRecord, ...]]:
    assets_by_paper: dict[str, list[AssetRecord]] = {}
    for paper_record in annotation_records:
        paper_id = str(paper_record.get("paper_id"))
        chunk_records = paper_record.get("chunk_annotation_coverage", [])
        if not isinstance(chunk_records, list):
            continue
        for chunk_record in chunk_records:
            if not isinstance(chunk_record, dict):
                continue
            asset_type = _asset_type_for_chunk(chunk_record)
            if asset_type is None:
                continue
            chunk_id = str(chunk_record.get("chunk_id"))
            span_record = structure_spans.get(paper_id, {}).get(chunk_id)
            warning_codes = {str(code) for code in chunk_record.get("warning_codes", []) if code is not None}
            warning_codes.add("linked_not_extracted")
            if span_record is None:
                warning_codes.add("missing_source_span")
            assets_by_paper.setdefault(paper_id, []).append(
                AssetRecord(
                    asset_id=f"{paper_id}:asset-link:{asset_type}:{len(assets_by_paper.get(paper_id, [])) + 1:04d}",
                    paper_id=paper_id,
                    asset_type=asset_type,
                    extraction_state="linked_not_extracted",
                    chunk_id=chunk_id,
                    source_artifact=f"normalized_markdown:{paper_id}",
                    source_span=_span_from_record(span_record) if span_record is not None else None,
                    provenance={
                        "created_from": "s04_annotation_sidecar_diagnostics",
                        "chunk_type": chunk_record.get("chunk_type"),
                        "route": chunk_record.get("route"),
                        "state": chunk_record.get("state"),
                        "annotation_types": list(chunk_record.get("annotation_types", [])),
                        "confidence_classes": list(chunk_record.get("confidence_classes", [])),
                    },
                    warning_codes=tuple(sorted(warning_codes)),
                )
            )
    return {paper_id: tuple(assets) for paper_id, assets in sorted(assets_by_paper.items())}


def _asset_type_for_chunk(chunk_record: dict[str, Any]) -> AssetType | None:
    chunk_type = str(chunk_record.get("chunk_type"))
    route = str(chunk_record.get("route"))
    if chunk_type in ASSET_CHUNK_TYPES:
        return ASSET_CHUNK_TYPES[chunk_type]
    if route == "citation_graph":
        return "reference"
    if route == "metadata_graph":
        return "metadata"
    return None


def _asset_record_with_source_file(asset: AssetRecord, *, source_file_by_role: dict[str, dict[str, Any]]) -> AssetRecord:
    source_file = source_file_by_role.get("normalized_markdown") or source_file_by_role.get("original_pdf")
    warning_codes = set(asset.warning_codes)
    source_file_id = None
    source_artifact = asset.source_artifact
    if source_file is None:
        warning_codes.add("missing_preserved_source_file")
    else:
        source_file_id = _string_or_none(source_file.get("source_file_id"))
        source_artifact = str(source_file.get("workspace_path"))
    return AssetRecord(
        asset_id=asset.asset_id,
        paper_id=asset.paper_id,
        asset_type=asset.asset_type,
        extraction_state=asset.extraction_state,
        source_file_id=source_file_id,
        chunk_id=asset.chunk_id,
        source_artifact=source_artifact,
        source_span=asset.source_span,
        workspace_path=asset.workspace_path,
        sha256=asset.sha256,
        byte_size=asset.byte_size,
        media_type=asset.media_type,
        provenance=asset.provenance,
        warning_codes=tuple(sorted(warning_codes)),
    )


def _chunk_source_spans_by_paper(path: Path) -> dict[str, dict[str, dict[str, Any]]]:
    by_paper: dict[str, dict[str, dict[str, Any]]] = {}
    for record in _load_jsonl(path):
        paper_id = str(record.get("paper_id"))
        chunks = record.get("chunk_diagnostics", [])
        if not isinstance(chunks, list):
            continue
        for chunk in chunks:
            if not isinstance(chunk, dict):
                continue
            chunk_id = _string_or_none(chunk.get("chunk_id"))
            span = chunk.get("source_span")
            if chunk_id is not None and isinstance(span, dict):
                by_paper.setdefault(paper_id, {})[chunk_id] = span
    return by_paper


def _span_from_record(record: dict[str, Any]) -> SourceSpan:
    bbox = record.get("bbox")
    return SourceSpan(
        coordinate_space=str(record["coordinate_space"]),
        char_start=int(record["char_start"]),
        char_end=int(record["char_end"]),
        page_start=int(record["page_start"]) if record.get("page_start") is not None else None,
        page_end=int(record["page_end"]) if record.get("page_end") is not None else None,
        bbox=tuple(float(value) for value in bbox) if isinstance(bbox, list) and len(bbox) == 4 else None,
    )


def _source_candidates_for_paper(paper: dict[str, Any]) -> tuple[SourceArtifactCandidate, ...]:
    paper_id = str(paper["paper_id"])
    explicit_paths = [Path(str(path)) for path in paper.get("required_paths", [])]
    markdown_path = _first_existing(
        [
            path if path.name in {"full_text.md", f"{paper_id}.md"} else path / "full_text.md"
            for path in explicit_paths
        ]
        + [Path("/root/.research/papers") / paper_id / "full_text.md", Path("/root/.arxiv_cache") / f"{paper_id}.md"]
    )
    pdf_path = _first_existing(
        [path if path.suffix.lower() == ".pdf" else path / f"{paper_id}.pdf" for path in explicit_paths]
        + [Path("/root/.arxiv_cache") / f"{paper_id}.pdf"]
    )
    return (
        SourceArtifactCandidate(
            source_role="normalized_markdown",
            path=markdown_path,
            original_reference=str(markdown_path) if markdown_path is not None else f"normalized_markdown:{paper_id}",
            missing_code=None if markdown_path is not None else "missing_normalized_markdown",
        ),
        SourceArtifactCandidate(
            source_role="original_pdf",
            path=pdf_path,
            original_reference=str(pdf_path) if pdf_path is not None else f"original_pdf:{paper_id}",
            missing_code=None if pdf_path is not None else "missing_original_pdf",
        ),
    )


def _first_existing(paths: list[Path]) -> Path | None:
    for path in paths:
        if path.is_file():
            return path
    return None


def _stable_source_name(*, source_role: SourceRole, source_path: Path) -> str:
    if source_role == "original_pdf":
        return "original.pdf"
    if source_role == "normalized_markdown":
        return "normalized.md"
    suffix = source_path.suffix or ".bin"
    return f"derived{suffix}"


def _sha256_file(path: Path) -> str:
    digest = sha256()
    with Path(path).open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _media_type_for_path(path: Path) -> str:
    suffix = path.suffix.lower()
    if suffix == ".pdf":
        return "application/pdf"
    if suffix in {".md", ".markdown"}:
        return "text/markdown"
    if suffix == ".json":
        return "application/json"
    return "application/octet-stream"


def _summary_for_manifests(manifests: tuple[dict[str, Any], ...], *, source_manifest: Path) -> dict[str, Any]:
    source_file_count = sum(len(manifest.get("source_files", [])) for manifest in manifests)
    asset_count = sum(len(manifest.get("assets", [])) for manifest in manifests)
    missing_counts: dict[str, int] = {}
    media_type_counts: dict[str, int] = {}
    asset_counts_by_type: dict[str, int] = {}
    extraction_state_counts: dict[str, int] = {}
    hash_count = 0
    for manifest in manifests:
        for warning in manifest.get("warnings", []):
            if isinstance(warning, dict):
                key = str(warning.get("code"))
                missing_counts[key] = missing_counts.get(key, 0) + 1
        for source_file in manifest.get("source_files", []):
            if not isinstance(source_file, dict):
                continue
            media_type = str(source_file.get("media_type"))
            media_type_counts[media_type] = media_type_counts.get(media_type, 0) + 1
            if _valid_sha256(source_file.get("sha256")):
                hash_count += 1
        for asset in manifest.get("assets", []):
            if not isinstance(asset, dict):
                continue
            asset_type = str(asset.get("asset_type"))
            state = str(asset.get("extraction_state"))
            asset_counts_by_type[asset_type] = asset_counts_by_type.get(asset_type, 0) + 1
            extraction_state_counts[state] = extraction_state_counts.get(state, 0) + 1
    return {
        "schema_version": "m005-source-preservation-run.v1",
        "source_manifest": str(source_manifest),
        "paper_count": len(manifests),
        "valid_manifest_count": sum(1 for manifest in manifests if validate_source_asset_manifest(manifest).valid_manifest),
        "source_file_count": source_file_count,
        "asset_count": asset_count,
        "hash_coverage_rate": hash_count / source_file_count if source_file_count else 0.0,
        "media_type_counts": dict(sorted(media_type_counts.items())),
        "asset_counts_by_type": dict(sorted(asset_counts_by_type.items())),
        "extraction_state_counts": dict(sorted(extraction_state_counts.items())),
        "missing_counts": dict(sorted(missing_counts.items())),
        "raw_text_included": False,
        "chunk_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def _manifest_to_record(manifest: dict[str, Any]) -> dict[str, Any]:
    validation = validate_source_asset_manifest(manifest)
    diagnostics = manifest.get("diagnostics", {})
    return {
        "schema_version": "m005-source-asset-package-diagnostic.v1",
        "paper_id": manifest.get("paper_id"),
        "valid_manifest": validation.valid_manifest,
        "source_file_count": len(manifest.get("source_files", [])),
        "asset_count": len(manifest.get("assets", [])),
        "hash_coverage_rate": diagnostics.get("hash_coverage_rate"),
        "source_files": [
            {
                "source_file_id": source_file.get("source_file_id"),
                "source_role": source_file.get("source_role"),
                "workspace_path": source_file.get("workspace_path"),
                "sha256": source_file.get("sha256"),
                "byte_size": source_file.get("byte_size"),
                "media_type": source_file.get("media_type"),
            }
            for source_file in manifest.get("source_files", [])
            if isinstance(source_file, dict)
        ],
        "warning_counts": diagnostics.get("warning_counts", {}),
        "raw_text_included": False,
        "chunk_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        value = json.loads(line)
        if isinstance(value, dict):
            records.append(value)
    return records


def _json_dumps(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def _validate_redaction(payload: dict[str, Any], *, object_id: str | None, object_type: str) -> list[AssetDiagnostic]:
    return _validate_nested_redaction(payload, object_id=object_id, object_type=object_type, path=())


def _validate_nested_redaction(value: Any, *, object_id: str | None, object_type: str, path: tuple[str, ...]) -> list[AssetDiagnostic]:
    diagnostics: list[AssetDiagnostic] = []
    if isinstance(value, dict):
        forbidden_fields = (
            FORBIDDEN_RAW_FIELDS
            | FORBIDDEN_EMBEDDING_FIELDS
            | FORBIDDEN_VECTOR_FIELDS
            | FORBIDDEN_SECRET_FIELDS
            | FORBIDDEN_OPTIMIZER_FIELDS
        ) & set(value)
        for field_name in sorted(forbidden_fields):
            if field_name in FORBIDDEN_RAW_FIELDS:
                reason = "raw_content_leakage"
            elif field_name in FORBIDDEN_EMBEDDING_FIELDS:
                reason = "embedding_leakage"
            elif field_name in FORBIDDEN_VECTOR_FIELDS:
                reason = "vector_leakage"
            elif field_name in FORBIDDEN_SECRET_FIELDS:
                reason = "secret_leakage"
            else:
                reason = "optimizer_trace_leakage"
            diagnostics.append(
                AssetDiagnostic(
                    reason=reason,
                    object_id=_redaction_path(object_id=object_id, object_type=object_type, path=(*path, str(field_name))),
                    object_type=object_type,
                )
            )
        for key, nested_value in value.items():
            diagnostics.extend(_validate_nested_redaction(nested_value, object_id=object_id, object_type=object_type, path=(*path, str(key))))
    elif isinstance(value, list):
        for index, nested_value in enumerate(value):
            diagnostics.extend(_validate_nested_redaction(nested_value, object_id=object_id, object_type=object_type, path=(*path, str(index))))
    return diagnostics


def _validate_required_fields(
    payload: dict[str, Any],
    *,
    fields: tuple[str, ...],
    object_id: str | None,
    object_type: str,
) -> list[AssetDiagnostic]:
    diagnostics: list[AssetDiagnostic] = []
    for field_name in fields:
        if field_name not in payload or payload.get(field_name) is None:
            diagnostics.append(AssetDiagnostic(reason=f"missing_{field_name}", object_id=object_id, object_type=object_type))
    return diagnostics


def _redaction_path(*, object_id: str | None, object_type: str, path: tuple[str, ...]) -> str:
    prefix = object_id or object_type
    return f"{prefix}:{'.'.join(path)}"


def _valid_source_span(value: Any) -> bool:
    return (
        isinstance(value, dict)
        and bool(value.get("coordinate_space"))
        and isinstance(value.get("char_start"), int)
        and isinstance(value.get("char_end"), int)
        and value["char_end"] > value["char_start"]
    )


def _valid_sha256(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(character in "0123456789abcdef" for character in value.lower())


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [str(item) for item in value if item is not None]


def _string_or_none(value: Any) -> str | None:
    return str(value) if value is not None else None


def _counts(values: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        if value is None:
            continue
        key = str(value)
        counts[key] = counts.get(key, 0) + 1
    return dict(sorted(counts.items()))
