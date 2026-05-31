"""Source asset preservation and registry package."""

from arxiv_archive.assets.registry import (  # noqa: F401
    AssetDiagnostic,
    AssetRecord,
    AssetValidationResult,
    PreservedSourceFile,
    SourceAssetManifest,
    SourceAssetRunResult,
    SourceArtifactCandidate,
    SourceSpan,
    attach_annotation_asset_links,
    build_source_asset_run,
    preserve_source_assets_for_paper,
    preserve_source_assets_manifest,
    validate_source_asset_manifest,
    write_source_asset_run,
)
from arxiv_archive.assets.provenance import annotation_asset_link_provenance, source_preservation_provenance  # noqa: F401
