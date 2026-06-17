"""Structure-aware paper chunking package."""

from research_graph.papers.chunking.chunker import (  # noqa: F401
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
    main,
    measure_structure_aware_manifest,
    parse_markdown_structure,
    write_structure_aware_run,
)
from research_graph.papers.chunking.figure_units import is_equation_block, is_figure_block  # noqa: F401
from research_graph.papers.chunking.table_units import is_table_block  # noqa: F401
