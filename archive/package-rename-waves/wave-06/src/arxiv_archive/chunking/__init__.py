"""Structure-aware chunking package."""

from arxiv_archive.chunking.chunker import (  # noqa: F401
    ChunkAnnotationSidecar,
    RouteEligibility,
    SourceSpan,
    StructuralElement,
    StructureAwareChunk,
    StructureAwareMeasurement,
    StructureAwarePackage,
    StructureAwareRunResult,
    build_structure_aware_package_for_paper,
    empty_structure_aware_package,
    measure_structure_aware_manifest,
    parse_markdown_structure,
    write_structure_aware_run,
)
from arxiv_archive.chunking.figure_units import is_equation_block, is_figure_block  # noqa: F401
from arxiv_archive.chunking.table_units import is_table_block  # noqa: F401
