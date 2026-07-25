"""Safety primitives for catalog-relative article paths and metadata flags."""

from __future__ import annotations

from collections.abc import Iterable
from pathlib import Path, PurePosixPath
from typing import Any


def normalize_posix_path(value: str) -> str:
    """Normalize slash style without granting traversal privileges."""

    return PurePosixPath(value.replace("\\", "/")).as_posix()


def catalog_root(catalog_path: Path) -> Path:
    """Return the resolved root directory for a catalog descriptor file."""

    return catalog_path.parent.resolve()


def safe_catalog_path(catalog_path: Path, article_path: str) -> Path:
    """Resolve a catalog-relative path that must stay under the catalog root."""

    normalized = normalize_posix_path(article_path)
    if normalized.startswith("/") or ".." in PurePosixPath(normalized).parts:
        raise ValueError(f"unsafe catalog-relative path: {article_path}")
    root = catalog_root(catalog_path)
    resolved = (catalog_path.parent / normalized).resolve()
    if not resolved.is_relative_to(root):
        raise ValueError(f"path resolves outside catalog root: {article_path}")
    return resolved


def article_ref_from_path(article_path: str, *, catalog_record_dir: str) -> str:
    """Extract a canonical article ref from a catalog article manifest path."""

    normalized = normalize_posix_path(article_path)
    prefix = f"{catalog_record_dir}/"
    suffix = "/article.json"
    if not normalized.startswith(prefix) or not normalized.endswith(suffix):
        raise ValueError(f"non-canonical article path: {article_path}")
    return normalized[len(prefix) : -len(suffix)]


def safety_flag_errors(
    location: str,
    value: Any,
    *,
    forbidden_true_flags: Iterable[str],
) -> list[str]:
    """Return fail-closed diagnostics for forbidden flags set to anything but false."""

    forbidden = set(forbidden_true_flags)
    errors: list[str] = []

    def visit(current_location: str, current_value: Any) -> None:
        if isinstance(current_value, dict):
            for key, child in current_value.items():
                child_location = f"{current_location}.{key}" if current_location else key
                if key in forbidden and child is not False:
                    errors.append(f"{child_location} must be false; got {child!r}")
                visit(child_location, child)
        elif isinstance(current_value, list):
            for index, child in enumerate(current_value):
                visit(f"{current_location}[{index}]", child)

    visit(location, value)
    return errors
