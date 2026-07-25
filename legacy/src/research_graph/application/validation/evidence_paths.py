"""Path primitives for validation evidence workflows."""

from __future__ import annotations

from pathlib import Path


class ValidationEvidencePathError(ValueError):
    """Raised when validation evidence paths are unsafe or malformed."""


def json_path(parent: str, key: str | int) -> str:
    """Return a stable JSON-pointer-like diagnostic path fragment."""

    return f"{parent}[{key}]" if isinstance(key, int) else f"{parent}.{key}"


def repo_relative_path(
    path_value: str | Path,
    *,
    repo_root: Path,
    label: str,
    require_exists: bool = True,
) -> Path:
    """Resolve a path that must stay under the repo root."""

    path_text = Path(path_value).as_posix() if isinstance(path_value, Path) else str(path_value)
    if not path_text or path_text.strip() != path_text or "://" in path_text:
        raise ValidationEvidencePathError(
            f"{label} must be a non-empty repo-relative path: {path_text!r}"
        )
    path = Path(path_text)
    if path.is_absolute() or any(part == ".." for part in path.parts):
        raise ValidationEvidencePathError(f"{label} must remain under the repo root: {path_text}")
    root = repo_root.resolve()
    resolved = (root / path).resolve()
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise ValidationEvidencePathError(f"{label} escapes repo root: {path_text}") from exc
    if require_exists and not resolved.exists():
        raise ValidationEvidencePathError(f"{label} is missing: {path_text}")
    return resolved


def safe_output_path(
    path_value: str | Path,
    *,
    repo_root: Path,
    label: str,
    output_dir: Path,
) -> Path:
    """Resolve a repo-relative output path that must stay under ``output_dir``."""

    path = repo_relative_path(path_value, repo_root=repo_root, label=label, require_exists=False)
    output_root = (repo_root / output_dir).resolve()
    try:
        path.resolve().relative_to(output_root)
    except ValueError as exc:
        raise ValidationEvidencePathError(
            f"{label} must be under {output_dir.as_posix()}: {path_value}"
        ) from exc
    return path
