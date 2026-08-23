"""Graph semantic query reader — validates .sgsnap content quality.

Reads a gzip-compressed Samyama snapshot and computes semantic metrics:
  - avg raw_text length per Section node (text completeness)
  - enrichment coverage (Author/Venue/Topic nodes per Paper)
  - entity dedup integrity (duplicate vids)
  - revision binding coverage (papers with revision-owned vids)
"""

from __future__ import annotations

import gzip
import json
import re
from pathlib import Path
from typing import Any

from governor.domain.trajectory import DriftFlag


class GraphQueryReader:
    """Extract semantic metrics from a .sgnap snapshot."""

    def __init__(self, root: Path):
        self.root = root

    def find_snapshots(self) -> list[Path]:
        """Find .sgnap files in data/graphs/ and artifacts/."""
        snapshots = []
        for pattern in ["data/graphs/*.sgsnap", "artifacts/**/*.sgsnap"]:
            snapshots.extend(self.root.glob(pattern))
        return sorted(set(snapshots))

    def compute_metrics(self, snapshot: Path) -> dict[str, Any]:
        """Compute all semantic metrics for one snapshot."""
        raw = gzip.open(snapshot, "rb").read()
        text = raw.decode(errors="replace")

        # Extract nodes by parsing JSON-like structures
        # sgsnap format has "labels" arrays and property maps
        metrics = {
            "snapshot": str(snapshot.relative_to(self.root)),
            "total_nodes": self._count_nodes(text),
            "total_edges": self._count_edges(text),
            "section_count": self._count_label(text, "Section"),
            "paper_count": self._count_label(text, "Paper"),
            "author_count": self._count_label(text, "Author"),
            "venue_count": self._count_label(text, "Venue"),
            "topic_count": self._count_label(text, "Topic"),
            "entity_count": self._count_label(text, "Entity"),
            "avg_raw_text_len": self._avg_raw_text(text),
            "duplicate_entity_vids": self._duplicate_vids(text, "Entity"),
            "revision_bound_papers": self._revision_bound(text),
        }
        return metrics

    def _count_nodes(self, text: str) -> int:
        return text.count('"schema_version"')

    def _count_edges(self, text: str) -> int:
        return text.count('"src"')

    def _count_label(self, text: str, label: str) -> int:
        """Count nodes with a specific label."""
        # sgsnap uses "labels":["Paper"] or similar
        pattern = rf'"{label}"'
        return len(re.findall(pattern, text)) // 2  # appears in labels + edges

    def _avg_raw_text(self, text: str) -> float:
        """Average length of raw_text values."""
        lengths = [len(m.group(1)) for m in re.finditer(
            r'"raw_text":"((?:[^"\\]|\\.)*)"', text
        )]
        return sum(lengths) / max(len(lengths), 1)

    def _duplicate_vids(self, text: str, label: str) -> int:
        """Count duplicate vids for a label."""
        vids = re.findall(
            rf'"vid":"((?:vid:{label.lower()}:)?[^"]+)"', text
        )
        # Filter to only vids matching this label
        matching = [v for v in vids if f"vid:{label.lower()}:" in v]
        return len(matching) - len(set(matching))

    def _revision_bound(self, text: str) -> int:
        """Count papers with revision-owned vids (contain 'vN' suffix)."""
        # Look for revision vid patterns like vid:paper:XXXX.NNNvN
        vids = re.findall(r'vid:paper:[0-9]+\.[0-9]+v[0-9]+', text)
        return len(set(vids))


def check_graph_semantics(root: Path) -> list[DriftFlag]:
    """Evaluate graph semantic quality against DATA thresholds."""
    import yaml

    contract_path = root / "scripts/governor/checks/graph_semantics.yaml"
    if not contract_path.exists():
        return []

    contract = yaml.safe_load(contract_path.read_text()) or {}
    if not contract:
        return []

    dimension = str(contract.get("dimension", "graph_semantics"))
    severity = str(contract.get("severity", "high"))

    def _flag(code: str, evidence: str) -> DriftFlag:
        return DriftFlag(flag=code, severity=severity, evidence=evidence, dimension=dimension)

    reader = GraphQueryReader(root)
    snapshots = reader.find_snapshots()

    if not snapshots:
        return [_flag("graph_semantics_no_snapshots",
                      "No .sgsnap snapshots found in data/graphs/ or artifacts/")]

    # Use the largest snapshot as primary
    primary = max(snapshots, key=lambda p: p.stat().st_size)
    metrics = reader.compute_metrics(primary)

    flags: list[DriftFlag] = []

    for rule in contract.get("rules", []):
        rule_id = rule.get("id", "unknown")
        threshold = rule.get("threshold", 0)
        description = rule.get("description", "")

        if rule_id == "section_text_completeness":
            avg = metrics["avg_raw_text_len"]
            if avg < threshold:
                flags.append(_flag(
                    "graph_section_text_incomplete",
                    f"avg raw_text = {avg:.0f} chars (threshold: {threshold}). "
                    f"Sections are stubs, not full text. {description}"
                ))

        elif rule_id == "enrichment_coverage":
            papers = metrics["paper_count"]
            enriched = metrics["author_count"] + metrics["venue_count"] + metrics["topic_count"]
            coverage = enriched / max(papers * 3, 1)  # 3 = author+venue+topic per paper
            if coverage < threshold:
                flags.append(_flag(
                    "graph_enrichment_missing",
                    f"enrichment coverage = {coverage:.1%} (threshold: {threshold:.0%}). "
                    f"Authors={metrics['author_count']}, Venues={metrics['venue_count']}, "
                    f"Topics={metrics['topic_count']} for {papers} papers. {description}"
                ))

        elif rule_id == "entity_dedup_integrity":
            dupes = metrics["duplicate_entity_vids"]
            if dupes > threshold:
                flags.append(_flag(
                    "graph_entity_duplication",
                    f"{dupes} duplicate Entity vids (threshold: {threshold}). {description}"
                ))

        elif rule_id == "revision_binding_coverage":
            papers = metrics["paper_count"]
            bound = metrics["revision_bound_papers"]
            coverage = bound / max(papers, 1)
            if coverage < threshold:
                flags.append(_flag(
                    "graph_revision_binding_low",
                    f"revision binding = {coverage:.1%} ({bound}/{papers}, "
                    f"threshold: {threshold:.0%}). {description}"
                ))

    return flags
