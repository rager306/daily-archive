"""Parser-run artifact helpers (M274 evidence foundation).

Application-pure helpers for content-addressed parser outputs.
Never authorizes import or graph writes.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "parser-run.v1"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_text(text: str) -> str:
    return sha256_bytes(text.encode("utf-8"))


def count_layout_elements(layout: Any) -> tuple[int, int]:
    """Return (element_count, bbox_count) from ODL-like JSON.

    Walks dict/list trees; counts objects with bbox/bounding_box/box keys.
    """
    elements = 0
    bboxes = 0

    def walk(node: Any) -> None:
        nonlocal elements, bboxes
        if isinstance(node, Mapping):
            elements += 1
            for key in ("bbox", "bounding_box", "boundingBox", "box", "coordinates"):
                if key in node and node[key] is not None:
                    bboxes += 1
                    break
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for item in node:
                walk(item)

    walk(layout)
    return elements, bboxes


@dataclass(frozen=True, slots=True)
class ParserRunManifest:
    schema_version: str
    paper_id: str
    parser: str
    parser_version: str | None
    config: dict[str, Any]
    artifact_paths: dict[str, str]
    content_hashes: dict[str, str]
    created_at: str
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("parser run cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "paper_id": self.paper_id,
            "parser": self.parser,
            "parser_version": self.parser_version,
            "config": dict(self.config),
            "artifact_paths": dict(self.artifact_paths),
            "content_hashes": dict(self.content_hashes),
            "created_at": self.created_at,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


def build_parser_run_manifest(
    *,
    paper_id: str,
    parser: str,
    artifact_paths: Mapping[str, str],
    content_hashes: Mapping[str, str],
    config: Mapping[str, Any] | None = None,
    parser_version: str | None = None,
    created_at: str | None = None,
) -> ParserRunManifest:
    return ParserRunManifest(
        schema_version=SCHEMA_VERSION,
        paper_id=paper_id,
        parser=parser,
        parser_version=parser_version,
        config=dict(config or {}),
        artifact_paths={str(k): str(v) for k, v in artifact_paths.items()},
        content_hashes={str(k): str(v) for k, v in content_hashes.items()},
        created_at=created_at
        or datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    )


def write_parser_run_manifest(path: Path, manifest: ParserRunManifest) -> Path:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(manifest.to_dict(), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return path


def write_bytes_artifact(path: Path, data: bytes) -> str:
    """Write bytes; return sha256 hex."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return sha256_bytes(data)


def write_text_artifact(path: Path, text: str) -> str:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return sha256_text(text)


__all__ = [
    "SCHEMA_VERSION",
    "ParserRunManifest",
    "build_parser_run_manifest",
    "count_layout_elements",
    "sha256_bytes",
    "sha256_text",
    "write_bytes_artifact",
    "write_parser_run_manifest",
    "write_text_artifact",
]
