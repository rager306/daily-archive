"""Adapter: filesystem artifact reader — reads JSON artifacts from disk."""

import json
from pathlib import Path
from typing import Iterator

from governor.ports.artifact_reader import ArtifactReader


class FilesystemArtifactReader(ArtifactReader):
    """Reads JSON artifact files from the filesystem."""

    def __init__(self, project_root: Path):
        self.root = project_root

    def iter_artifacts(self, base_dir: str = "artifacts/") -> Iterator[Path]:
        scan_root = self.root / base_dir
        if not scan_root.exists():
            return
        for path in sorted(scan_root.rglob("*.json")):
            if path.is_file():
                yield path

    def read_json(self, path: Path) -> dict:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)
