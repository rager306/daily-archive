"""Source provenance helpers for preserved assets.

The registry owns contract serialization; this module centralizes the small,
redacted provenance dictionaries passed across preservation boundaries.
"""

from __future__ import annotations

from typing import Any


def source_preservation_provenance(*, original_reference: str, source: str = "gold_manifest_required_path") -> dict[str, Any]:
    """Return redacted provenance metadata for copied source artifacts."""
    return {
        "source": source,
        "original_reference": original_reference,
        "preservation_method": "copy2",
    }


def annotation_asset_link_provenance(chunk_record: dict[str, Any]) -> dict[str, Any]:
    """Return redacted provenance metadata for annotation-derived asset links."""
    return {
        "created_from": "s04_annotation_sidecar_diagnostics",
        "chunk_type": chunk_record.get("chunk_type"),
        "route": chunk_record.get("route"),
        "state": chunk_record.get("state"),
        "annotation_types": list(chunk_record.get("annotation_types", [])),
        "confidence_classes": list(chunk_record.get("confidence_classes", [])),
    }
