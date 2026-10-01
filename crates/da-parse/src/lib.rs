//! Scholarly PDF fulltext parse plane.
//!
//! This crate owns Java GROBID HTTP, this-cut TEI IR, and the `ParserPort`
//! projector onto `da_ports::parser::ParsedArticle`. HTML parse, PDF
//! classification, and XY-Cut stay in `da-adapters`.
//!
//! Since M464-z84in2 S02 the transport is behind the provider-neutral
//! `ParseBackend` seam (`TeiEnvelope` in, exact backend/profile identity out),
//! so cached-TEI replay and the GROBID HTTP path share one contract (R105).
//! [`ScholarlyParser`] closes the seams into the runtime
//! backend→envelope→lift→project composition; [`GrobidParser`] stays the
//! compatibility facade for existing `Arc<dyn ParserPort>` callers. Since
//! S04/T01 the request carries the serde-versioned V1 fulltext dialect and
//! the transport returns the closed typed [`ParseBackendFailure`] vocabulary
//! (R107); the compatibility `ParserError` mapping stays exactly once at the
//! facade boundary. Since S05/T01 the same seam covers the closed header
//! and references operations: versioned per-endpoint request dialects and
//! endpoint result identities distinguish every operation and effective
//! request without touching the fulltext facade or the D204 pins (R113).
//! Since S06/T01 the frozen bounded-batch and cache contract lives in
//! [`batch`] / [`cache`]: the closed four-mode cache vocabulary with its
//! truth table, NonZero bounded limits, ordered items over
//! [`ParseBackendRequest`], the ordered input fingerprint, the schema-bound
//! resume cursor, per-stage dispositions and one typed terminal outcome per
//! admitted document — still without touching `ParserPort` or introducing
//! any path/env/retry surface.

mod backend;
mod batch;
mod batch_budget;
mod cache;
mod callout_markers;
mod conservation;
mod cross_stage_semantic_invariants;
mod debug_tape;
mod envelope;
mod http;
mod ir;
mod java_invocation;
mod layout;
mod layout_lattice;
mod lift;
mod managed_batch;
mod native_affiliation;
mod native_bibliography_features;
mod native_bibliography_gazetteer;
mod native_bibliography_native_input;
mod native_bibliography_nested;
mod native_bibliography_sequence;
mod native_body;
mod native_body_order_lineage;
mod native_body_text;
mod native_callout_classification;
mod native_callout_matching;
mod native_citation;
mod native_composition;
mod native_consolidation_specialized;
mod native_date;
mod native_feature_facts;
mod native_feature_tagger;
mod native_figure_sequence;
mod native_formula_join;
mod native_fulltext_blocks;
mod native_fulltext_composition;
mod native_fulltext_features;
mod native_fulltext_reading_order;
mod native_fulltext_sentence;
mod native_fulltext_sequence;
mod native_funding_features;
mod native_funding_sequence;
mod native_header_features;
mod native_header_fields;
mod native_header_native_input;
mod native_header_sequence;
mod native_heading_regions;
mod native_personal_name;
mod native_region_input;
mod native_segmentation;
mod native_table_sequence;
mod native_visual;
mod native_visual_family_cluster;
mod native_word_projection;
mod p5_shadow_join;
mod parser;
mod profile;
mod projector;
mod stage_tape;
mod transport;

pub use backend::{
    CACHED_TEI_BACKEND_ID, CachedTeiBackend, GROBID_HTTP_BACKEND_ID, ParseBackend,
    ParseBackendRequest,
};
pub use batch::{
    BatchCheckpoint, BatchCursor, PARSE_BATCH_CURSOR_SCHEMA_VERSION,
    PARSE_BATCH_RUN_SUMMARY_SCHEMA_VERSION, PARSE_BATCH_SCHEMA_VERSION, ParseBatch,
    ParseBatchAdmissionError, ParseBatchBudget, ParseBatchCancellation, ParseBatchCollector,
    ParseBatchCursorError, ParseBatchExecutor, ParseBatchItem, ParseBatchLimits,
    ParseBatchMemoryCheckpoint, ParseBatchResultSink, ParseBatchRunSummary, ParseBatchSinkError,
    ParseDocumentFailure, ParseDocumentOutcome, ParseDocumentSuccess, ordered_input_fingerprint,
    ordered_input_fingerprint_with_backend_version,
};
pub use batch_budget::{
    BATCH_BUDGET_ABSENCE_REQUIRES_REASON, BATCH_BUDGET_CEILINGS_ADR_REF,
    BATCH_BUDGET_CEILINGS_REQUIREMENT_IDS, BATCH_BUDGET_CEILINGS_SCHEMA_VERSION,
    BATCH_BUDGET_COUNT_LIMIT_IS_NOT_A_BYTE_CEILING, BATCH_BUDGET_H05_CLOSURE,
    BATCH_BUDGET_H05_REASON, BATCH_BUDGET_METRICS, BATCH_BUDGET_NO_LATENCY_OBSERVATION_REASON,
    BATCH_BUDGET_NO_RETENTION_CEILING_REASON, BATCH_BUDGET_RETENTION_CEILING_COMPOSES_WITH,
    BATCH_BUDGET_SILENT_CLAMP, BATCH_BUDGET_VERDICTS, BATCH_BUDGET_ZERO_IS_NOT_A_MEASUREMENT,
    BATCH_OBSERVATION_STATES, BackendTerminationClass, BatchBudgetVerdict, BatchObservation,
    ParseBatchBudgetObservation, classify_backend_termination,
};
pub use cache::{
    CacheIntegrityCode, CacheMode, CacheStageDisposition, MEMORY_TIER_DEFAULT_CAPACITY,
    PARSE_CACHE_DISPOSITION_SCHEMA_VERSION, PARSE_CACHE_FAILURE_SCHEMA_VERSION,
    PROJECT_ARTICLE_PAYLOAD_SCHEMA_VERSION, PROJECT_ARTICLE_SCHEMA_FINGERPRINT,
    PROJECT_ARTICLE_STAGE_ID, ParseArtifactCache, ParseCacheDispositions, ParseCacheFailure,
    ParseCacheStage, ProjectArticlePayload, RAW_TEI_PAYLOAD_SCHEMA_VERSION, RAW_TEI_STAGE_ID,
    RawStageRead, RawTeiEnvelopePayload, TwoStageParseCache, project_article_stage_key,
    project_article_stage_key_with_backend_version, raw_tei_stage_key,
    raw_tei_stage_key_with_backend_version,
};
pub use callout_markers::{
    CALLOUT_MARKER_SCHEMA_VERSION, CalloutBodyInventory, CalloutBodyTarget, CalloutLabelBasis,
    CalloutLabelKind, CalloutLink, CalloutLinkReport, CalloutLinkTarget, CalloutLinkUnresolved,
    CalloutMarkerClass, CalloutMarkerError, CalloutMarkerOccurrence, MAX_CALLOUT_MARKERS,
    MAX_CALLOUT_RANGE_SPAN, MAX_CALLOUT_TEXT_BYTES, classify_and_link_callout_markers,
    classify_callout_markers, link_callout_markers,
};
pub use conservation::{
    ByteRange, ConservationDelta, ConservationEngine, ConservationEntry, ConservationFamily,
    ConservationIdentity, ConservationInventory, ConservationObservation, ConservationReasonCode,
    ConservationSubtype, ConservationVerdict, FamilyDeltaCounts,
};
pub use cross_stage_semantic_invariants::{
    CROSS_STAGE_SEMANTIC_SCHEMA_VERSION, CrossStageSemanticInvariantsError,
    CrossStageSemanticInvariantsInput, CrossStageSemanticInvariantsReport, JavaP4Observation,
    JavaP6Observation, JavaP7Observation, NativeBucketReport, NativeVisualHeadReport,
    check_cross_stage_semantic_invariants,
};
pub use debug_tape::{DebugSection, model_invocation_counts, parse_debug_sections};
pub use envelope::{TeiEnvelope, TeiIdentity, TeiResultIdentity};
pub use http::{
    GrobidHttpBackend, GrobidParser, HealthProbe, ProbeFailure, VersionProbe,
    grobid_model_fingerprint,
};
pub use ir::TeiParseIr;
pub use java_invocation::{JavaInvocationCounter, JavaInvocationObservation};
pub use layout::{
    LAYOUT_SCHEMA_VERSION, LayoutBBox, LayoutDocument, LayoutError, LayoutPage, LayoutProvider,
    LayoutSpan, LayoutSpanAbsence, LayoutStyleHints, LayoutToken, LayoutTokenKind,
};
pub use layout_lattice::{
    CoordinateDialect, FurnitureEvidence, FurnitureKind, LAYOUT_LATTICE_SCHEMA_VERSION,
    LatticeBlock, LatticeIdentity, LatticeLine, LatticePage, LatticeToken, LayoutLattice,
    LayoutLatticeError, MAX_LATTICE_BLOCKS, MAX_LATTICE_LINES, MAX_LATTICE_PAGES, ReadingOrder,
};
pub use lift::TeiLifter;
pub use managed_batch::ManagedParseBatch;
pub use native_affiliation::{
    AffiliationComponentKind, AffiliationSpan, NATIVE_AFFILIATION_INPUT_BYTE_CAP,
    NATIVE_AFFILIATION_LINK_SCHEMA_VERSION, NATIVE_AFFILIATION_OUTCOME_SCHEMA_VERSION,
    NATIVE_AFFILIATION_PARSER_ID, NATIVE_AFFILIATION_PARSER_VERSION, NativeAffiliationAmbiguous,
    NativeAffiliationAuthorEvidence, NativeAffiliationLink, NativeAffiliationLinkError,
    NativeAffiliationLinkOutcome, NativeAffiliationLinker, NativeAffiliationMalformed,
    NativeAffiliationNesting, NativeAffiliationOccurrence, NativeAffiliationOutcome,
    NativeAffiliationParser, NativeAffiliationUnsupported, RecognizedAffiliationComponent,
};
pub use native_bibliography_features::{
    CitationFeatureRow, NATIVE_BIBLIOGRAPHY_FEATURES_SCHEMA_VERSION,
    NATIVE_CITATION_FEATURE_COLUMNS, NATIVE_CITATION_LABEL_COUNT, NATIVE_CITATION_LABELS,
    NATIVE_CITATION_MODEL_MAX_COLUMN_INDEX, NATIVE_CITATION_ROLE_COUNT,
    NATIVE_REFERENCE_SEGMENTER_FEATURE_COLUMNS, NATIVE_REFERENCE_SEGMENTER_LABEL_COUNT,
    NATIVE_REFERENCE_SEGMENTER_LABELS, NATIVE_REFERENCE_SEGMENTER_MODEL_MAX_COLUMN_INDEX,
    NATIVE_TRAINING_LABEL_PLACEHOLDER, NativeBibliographyFeatureError,
    ReferenceSegmenterFeatureRow, citation_row, reference_segmenter_row,
    render_citation_feature_text, render_reference_segmenter_feature_text,
};
pub use native_bibliography_gazetteer::{
    BibliographyGazetteerError, BibliographyGazetteerIndex, BibliographyGazetteerLane,
    BibliographyGazetteerSpans, GAZETTEER_DELIMITERS_HEAD, GAZETTEER_MAX_ENTRIES_PER_LANE,
    GAZETTEER_MAX_PHRASE_TOKENS, GAZETTEER_MAX_SPANS, GAZETTEER_MAX_TEXT_BYTES,
    GAZETTEER_MAX_TOKENS, GazetteerSpan, NATIVE_BIBLIOGRAPHY_GAZETTEER_SCHEMA_VERSION,
    gazetteer_spans, is_isbn_shape, is_issn_shape, span_covers,
};
pub use native_bibliography_native_input::{
    BibliographyAtom, BibliographyAtomDisposition, BibliographyCoverageVerdict,
    BibliographyLineFacts, BibliographyOracleIndependenceError, BibliographyOracleProvenance,
    NATIVE_BIBLIOGRAPHY_INDENT_EPSILON_PT, NATIVE_BIBLIOGRAPHY_INPUT_GENERATOR_ID,
    NATIVE_BIBLIOGRAPHY_INPUT_SCHEMA_VERSION, NativeBibliographyAnchorScope,
    NativeBibliographyAtomOutcome, NativeBibliographyCrfScope, NativeBibliographyFieldCount,
    NativeBibliographyFromInputOutcome, NativeBibliographyInputError,
    NativeBibliographyInputIdentity, NativeBibliographyOutOfScope, bibliography_char_set,
    build_bibliography_line_facts, build_native_bibliography_input, classify_bibliography_coverage,
    normalize_bibliography_text, require_independent_bibliography_oracle,
};
pub use native_bibliography_nested::{
    DATE_PART_BASE_LABELS, DateFeatureRow, DatePartRole, NAME_PART_BASE_LABELS,
    NATIVE_BIBLIOGRAPHY_NESTED_SCHEMA_VERSION, NATIVE_CITATION_IDENTIFIER_COLUMN,
    NATIVE_DATE_FEATURE_COLUMNS, NATIVE_DATE_LABEL_COUNT, NATIVE_DATE_LABELS,
    NATIVE_DATE_MODEL_MAX_COLUMN_INDEX, NATIVE_DATE_REFERENCED_COLUMNS,
    NATIVE_DATE_SEQUENCE_FAMILY, NATIVE_NAME_CITATION_FEATURE_COLUMNS,
    NATIVE_NAME_CITATION_LABEL_COUNT, NATIVE_NAME_CITATION_LABELS,
    NATIVE_NAME_CITATION_MODEL_MAX_COLUMN_INDEX, NATIVE_NAME_CITATION_REFERENCED_COLUMNS,
    NATIVE_NAME_CITATION_SEQUENCE_FAMILY, NATIVE_NESTED_UNREFERENCED_LABEL_PLACEHOLDER,
    NameCitationFeatureRow, NamePartRole, NativeBibliographyNestedError, NativeDatePart,
    NativeDateValue, NativeIdentifierEvidence, NativeNameCitation, NativeNamePart,
    NativePersonTitleCoverage, cluster_identifier, date_row, decode_date_cluster,
    decode_name_citation_cluster, name_citation_row, render_date_feature_text,
    render_name_citation_feature_text,
};
pub use native_bibliography_sequence::{
    BibliographyLabelRun, BibliographyLabelSequence, BibliographySequenceFamily,
    CITATION_ROLE_BASE_LABELS, CitationClusterSequence, CitationRole, CitationRoleSpan,
    MAX_NATIVE_BIBLIOGRAPHY_SEQUENCE_ROWS, NATIVE_BIBLIOGRAPHY_SEQUENCE_SCHEMA_VERSION,
    NATIVE_CITATION_SEQUENCE_FAMILY, NATIVE_REFERENCE_SEGMENTER_SEQUENCE_FAMILY,
    NativeBibliographySequenceError, ReferenceSegmentKind, ReferenceSegmentSequence,
    ReferenceSegmentSpan, bibliography_label_runs, decode_bibliography_labels,
    decode_citation_sequence, decode_reference_sequence,
};
pub use native_body::{
    AccountedKind, AccountedUnit, AccountingBucket, BucketCount, JavaFallbackEvidence,
    MAX_ACCOUNTED_ASSIGNMENTS, NATIVE_BODY_SCHEMA_VERSION, NATIVE_BODY_SEQUENCE_SCHEMA_VERSION,
    NativeBodyAccountant, NativeBodyAccounting, NativeBodyError, NativeBodyLimits,
    NativeBodySequenceCounts, NativeBodySequenceIdentity, NativeBodySequenceInput,
    NativeBodySequenceReceipt, NativeBodySequenceStep, run_native_body_sequence, source_digest,
};
pub use native_body_order_lineage::{
    BodyOrderEntry, BodyOrderLane, BodyOrderLaneAudit, BodyOrderLaneDisposition, BodyOrderLineage,
    BodyOrderLineageAudit, BodyOrderLineageError, BodyOrderPairAudit, DocumentTokenId,
    MAX_ORDER_LINEAGE_ENTRIES, NATIVE_BODY_ORDER_LINEAGE_SCHEMA_VERSION,
    SEGMENTER_REJECTED_LATTICE, audit_body_order_lineage, ordered_fragments,
};
pub use native_body_text::{
    BODY_ABSENCE_POLICY, BODY_ALIGNMENT_METHOD, BODY_GAP_THRESHOLDS, BodyAbsentDocument,
    BodyAggregateInput, BodyAssignmentRoute, BodyBoundaryVerification, BodyCasePolicy,
    BodyContentAlignment, BodyCoverageAggregate, BodyCoverageGap, BodyCoverageVerdict,
    BodyDocumentOutcome, BodyGapReason, BodyJoinPolicy, BodyNormalization, BodyOracleCounts,
    BodyOracleIndependence, BodyOracleProvenance, BodyPlaneLocalization, BodyPlaneVerdict,
    BodyRegionVerdict, BodyScope, BodyScopeClass, BodyScopeSide, BodyThirdRouteProvenance,
    BodyThirdRouteVerdict, BodyUncertaintySplit, BodyUnitInventory, BodyUnitRung,
    MAX_BOUNDARY_TILE_UNITS, MAX_NATIVE_BODY_TOKENS, NATIVE_BODY_ASSIGNMENT_SCHEMA_VERSION,
    NATIVE_BODY_TEXT_GENERATOR_ID, NATIVE_BODY_TEXT_SCHEMA_VERSION,
    NATIVE_BODY_THIRD_ROUTE_SCHEMA_VERSION, NativeBodyAssignment, NativeBodyLengths,
    NativeBodyText, NativeBodyTextError, NativeBodyThirdRouteCandidate, aggregate_body_coverage,
    align_body_content, build_native_body_assignment, build_native_body_text,
    classify_body_coverage, classify_body_plane, classify_body_region,
    evaluate_body_third_route_candidate, localize_body_plane_checked, plane_verdict_for_population,
    require_independent_body_oracle, scope_covers, split_body_uncertainty, split_plane_uncertainty,
};
pub use native_callout_classification::{
    CALLOUT_DENOMINATOR_DIGEST_DOMAIN, CALLOUT_OBSERVATION_DIGEST_DOMAIN, CalloutDenominator,
    CalloutInventory, CalloutLabelKeyConflict, CalloutLabelOrdinalEntry, CalloutLabelOrdinalMap,
    CalloutLabelResolution, CalloutLinkageVerdict, CalloutMarkerClassification, CalloutMarkerInput,
    CalloutMarkerObservation, CalloutMarkerType, CalloutStyleCount, CalloutStyleMajority,
    CalloutToken, MAX_NATIVE_CALLOUT_MARKER_TOKENS, MAX_NATIVE_CALLOUT_MARKERS,
    MAX_NATIVE_CALLOUT_REFERENCE_LINES, MEM992_FROZEN_TARGETED_CALLOUTS,
    MEM992_FROZEN_TARGETLESS_CALLOUTS, MEM992_FROZEN_TOTAL_CALLOUTS,
    NATIVE_CALLOUT_CLASSIFICATION_SCHEMA_VERSION, NATIVE_CALLOUT_INVENTORY_LANE,
    NATIVE_CALLOUT_TARGETED_DENOMINATOR, NATIVE_CALLOUT_TARGETLESS_DENOMINATOR,
    NativeCalloutClassificationError, NativeCalloutResolution, build_callout_inventory,
    build_callout_label_ordinal_map, callout_linkage_verdict, classify_callout_marker,
    majority_callout_style, marker_adjacency, normalize_callout_label,
};
pub use native_callout_matching::{
    AdjacencyDisposition, CalloutMatch, CalloutPosition, CalloutTargetClass,
    CalloutUnresolvedReason, DigestOnlyCalloutTarget, DigestOnlyTargetInventory,
    MAX_CALLOUT_OBSERVATIONS, MajorityStyleDisposition, MajorityStyleOutcome,
    NATIVE_CALLOUT_MATCHING_SCHEMA_VERSION, NativeCalloutMatchingError,
    NativeCalloutMatchingOutcome, NativeCalloutUpstreamStatus, ObservedCalloutStyle,
    inventory_digest, match_native_callouts,
};
pub use native_citation::{
    CitationField, CitationFieldRole, CitationSpan, NATIVE_CITATION_ANCHOR_SCANNER_ID,
    NATIVE_CITATION_INPUT_BYTE_CAP, NATIVE_CITATION_OUTCOME_SCHEMA_VERSION,
    NATIVE_CITATION_PARSER_ID, NATIVE_CITATION_PARSER_VERSION, NATIVE_REFERENCE_SEGMENTER_ID,
    NativeCitationAmbiguous, NativeCitationAnchor, NativeCitationAnchorError,
    NativeCitationAnchorLimits, NativeCitationAnchorOutcome, NativeCitationAnchorScanner,
    NativeCitationMalformed, NativeCitationOutcome, NativeCitationParser,
    NativeCitationUnsupported, NativeReference, NativeReferenceListError,
    NativeReferenceListOutcome, NativeReferenceSegmenter, NativeReferenceSegmenterLimits,
};
pub use native_composition::{
    ComponentDisposition, ComponentReceipt, CompositionComponent, CompositionError,
    CompositionSpan, HeaderAuthor, HeaderAuthorInput, HeaderCandidate, HeaderCompositionCandidate,
    HeaderCompositionInput, HeaderCompositionOutcome, HeaderSequenceInput,
    NATIVE_COMPOSITION_MAX_AFFILIATIONS, NATIVE_COMPOSITION_MAX_ANCHORS,
    NATIVE_COMPOSITION_MAX_AUTHORS, NATIVE_COMPOSITION_MAX_FIELDS,
    NATIVE_COMPOSITION_MAX_REFERENCES, NATIVE_COMPOSITION_SCHEMA_VERSION, ReferenceCandidate,
    ReferenceCompositionCandidate, ReferenceCompositionInput, ReferenceCompositionOutcome,
    ReferenceSequenceInput, compose_header, compose_header_with_fields, compose_references,
    run_header_sequence, run_header_sequence_with_fields, run_reference_sequence,
};
pub use native_consolidation_specialized::{
    DigestOnlyEnrichmentReceipt, MAX_SPECIALIZED_DIGESTS,
    NATIVE_CONSOLIDATION_SPECIALIZED_SCHEMA_VERSION, NativeConsolidationSpecializedError,
    NativeConsolidationSpecializedOutcome, SpecializedDisposition, SpecializedFamily,
    SpecializedFamilyOutcome, SpecializedUpstreamStatus, evaluate_native_consolidation_specialized,
};
pub use native_date::{
    DatePrecision, NATIVE_DATE_INPUT_BYTE_CAP, NATIVE_DATE_OUTCOME_SCHEMA_VERSION,
    NATIVE_DATE_PARSER_ID, NATIVE_DATE_PARSER_VERSION, NativeDateAmbiguous, NativeDateMalformed,
    NativeDateOutcome, NativeDateParser, NativeDateUnsupported, RecognizedDate,
};
pub use native_feature_facts::{
    BIBLIOGRAPHY_COMMON_NAME_CHANNEL, BIBLIOGRAPHY_FIRST_NAME_CHANNEL,
    BIBLIOGRAPHY_FULL_PUNCTUATIONS, BIBLIOGRAPHY_LAST_NAME_CHANNEL,
    BIBLIOGRAPHY_MAX_PUNCTUATION_PROFILE, BIBLIOGRAPHY_MONTHS, BibliographyAlignment,
    BibliographyNameFlags, BibliographyTokenFacts, BibliographyTokenInput,
    CITATION_RELATIVE_POSITION_BINS, Capitalisation, DigitStatus, FactsBlockStatus,
    FactsLineStatus, NATIVE_FEATURE_FACTS_SCHEMA_VERSION, PunctType,
    REFERENCE_SEGMENTER_LINE_SCALE, capitalisation, capitalisation_as_printed, contains_http_shape,
    digit_status, is_all_capital, is_month_name, is_punctuation_run, is_single_char, is_year_shape,
    linear_scaling, name_flags, punct_type, punctuation_profile, punctuation_profile_field,
    relative_position_citation, relative_position_reference_segmenter, token_prefix, token_suffix,
};
pub use native_feature_tagger::{
    FeatureDisposition, FeatureUpstreamStatus, NATIVE_FEATURE_COLUMNS,
    NATIVE_FEATURE_SCHEMA_VERSION, NativeFeatureError, NativeFeatureInput, NativeFeatureOutput,
    NativeFeatureTagger, StructuralFeatureRow,
};
pub use native_figure_sequence::{
    FigureSpan, MAX_NATIVE_FIGURE_ROWS, NATIVE_FIGURE_BASE_LABELS, NATIVE_FIGURE_EMITTED_COLUMNS,
    NATIVE_FIGURE_LABELS, NATIVE_FIGURE_MODEL_MAX_COLUMN_INDEX,
    NATIVE_FIGURE_SEQUENCE_SCHEMA_VERSION, NativeFigureAbsence, NativeFigureDecodeOutcome,
    NativeFigureSequenceError, check_figure_column_window, check_figure_label_block,
    decode_figure_sequence, figure_label_is_admissible, figure_spans_from_decoded,
};
pub use native_formula_join::{
    MAX_NATIVE_FORMULA_EQUATIONS, MAX_NATIVE_FORMULA_ROWS, NATIVE_FORMULA_JOIN_LABELS,
    NATIVE_FORMULA_JOIN_SCHEMA_VERSION, NATIVE_FORMULA_LOCATOR_LABELS, NATIVE_FORMULA_MARKER_LABEL,
    NativeEquationSpan, NativeFormulaAbsence, NativeFormulaJoinError, NativeFormulaJoinOutcome,
    is_formula_join_label, join_native_equation_labels, join_native_equations,
};
pub use native_fulltext_blocks::{
    AnnexParentRun, FullRegionAccountingUnit, FullRegionUnitKind, FulltextLeafAssignment,
    FulltextLeafRole, FulltextRoleHint, MAX_FULLTEXT_BLOCKS, MarkerSubtype,
    NATIVE_FULL_REGION_ACCOUNTING_SCHEMA_VERSION, NATIVE_FULLTEXT_BLOCKS_SCHEMA_VERSION,
    NativeFullRegionAccounting, NativeFulltextBlocksConfig, NativeFulltextBlocksError,
    NativeFulltextBlocksOutcome, NoteDisposition, account_full_regions, run_native_fulltext_blocks,
};
pub use native_fulltext_composition::{
    AdmissionDisposition, ByteAccountingLedger, ByteAccountingViolation, ByteCountScope,
    FulltextAdmissionReceipt, FulltextComponent, FulltextComponentManifest,
    FulltextCompositionIdentity, NATIVE_FULLTEXT_COMPOSITION_SCHEMA_VERSION,
    NATIVE_FULLTEXT_FALLBACK_TARGET, NATIVE_FULLTEXT_LANE_FALLBACK_ADR_REF,
    NATIVE_FULLTEXT_LANE_FALLBACK_CONTRACT_ID, NATIVE_FULLTEXT_LANE_FALLBACK_LANE_CLOSURE,
    NATIVE_FULLTEXT_LANE_FALLBACK_LANES, NATIVE_FULLTEXT_LANE_FALLBACK_POOLING,
    NATIVE_FULLTEXT_LANE_FALLBACK_RUST_RUN_MAY_NOT_CANCEL,
    NATIVE_FULLTEXT_LANE_FALLBACK_TERMINAL_RECEIPT_IS_PASS, NATIVE_FULLTEXT_MAX_COMPONENT_BYTES,
    NATIVE_FULLTEXT_MAX_JAVA_BYTES, NATIVE_FULLTEXT_MAX_XML_DEPTH,
    NATIVE_FULLTEXT_REFERENCE_LINK_DERIVATION, NATIVE_FULLTEXT_REFERENCE_LINK_ID_SPACE,
    NATIVE_FULLTEXT_REFERENCE_LINK_POSITIONAL_FALLBACK,
    NATIVE_FULLTEXT_REFERENCE_LINK_REQUIRED_TARGETS, NATIVE_FULLTEXT_REFERENCE_LINK_RESOLVE_GATE,
    NativeFulltextCandidate, NativeFulltextCompositionError, NativeFulltextCompositionInput,
    NativeFulltextCompositionOutcome, NativeFulltextManifest, ReferenceLinkLedger,
    S17LineageReceipt, SemanticFieldLedger, SemanticFieldLoss, citation_field_role_token,
    compose_native_fulltext, tei_element_bytes,
};
pub use native_fulltext_features::{
    FulltextAlignment, FulltextCalloutType, FulltextFeatureRow, FulltextFeatureToken,
    FulltextFontSize, FulltextFontStatus, MAX_NATIVE_FULLTEXT_FEATURE_ROWS,
    NATIVE_FULLTEXT_FEATURE_COLUMNS, NATIVE_FULLTEXT_FEATURE_SCHEMA_VERSION,
    NATIVE_FULLTEXT_MODEL_MAX_COLUMN_INDEX, NATIVE_FULLTEXT_RELATIVE_POSITION_BINS,
    NativeFulltextFeatureError, check_fulltext_feature_columns,
    relative_position_fulltext_document, relative_position_fulltext_page,
    render_fulltext_feature_text, render_fulltext_features,
};
pub use native_fulltext_reading_order::{
    ColumnShift, CrossPageRun, MAX_FULLTEXT_PAGES, MAX_FULLTEXT_TOKENS,
    NATIVE_FULLTEXT_READING_ORDER_SCHEMA_VERSION, NativeFulltextPageToken,
    NativeFulltextReadingOrder, NativeFulltextReadingOrderError, NativeFulltextTokenText,
    PageReadingSpan, cross_page_runs, glue_fulltext_reading_order,
};
pub use native_fulltext_sentence::{
    NATIVE_BODY_ADJACENT_SURFACES, NATIVE_FULLTEXT_SENTENCE_SCHEMA_VERSION,
    NATIVE_LANGUAGE_GAP_REASON, NATIVE_LANGUAGE_RESOURCE_NAME, NATIVE_SENTENCE_GAP_REASON,
    NATIVE_SENTENCE_RESOURCE_NAME, NativeBodyStep, NativeBodyStepOutcome, NativeBodyStepReceipt,
    NativeBodySurfaceKind, NativeBodyVersionSurface, NativeLanguageStep, NativeSentenceStep,
    native_body_version_surface,
};
pub use native_fulltext_sequence::{
    FulltextLabelRun, MAX_NATIVE_FULLTEXT_SEQUENCE_ROWS, NATIVE_FULLTEXT_BASE_LABELS,
    NATIVE_FULLTEXT_LABELS, NATIVE_FULLTEXT_SEQUENCE_SCHEMA_VERSION, NativeFulltextDecodeCandidate,
    NativeFulltextDecodeOutcome, NativeFulltextSequenceError, check_fulltext_label_block,
    decode_fulltext_sequence, fulltext_label_is_admissible,
};
pub use native_funding_features::{
    FundingCapitalisation, FundingDigit, FundingFeatureRow, FundingFeatureToken, FundingLineStatus,
    FundingPunctType, MAX_NATIVE_FUNDING_FEATURE_ROWS, MAX_NATIVE_FUNDING_GAZETTEER_SPANS,
    NATIVE_FUNDING_FEATURE_COLUMNS, NATIVE_FUNDING_FEATURES_SCHEMA_VERSION,
    NATIVE_FUNDING_LABEL_COLUMN_DUMMY, NATIVE_FUNDING_LABEL_COLUMN_INDEX,
    NATIVE_FUNDING_MODEL_MAX_COLUMN_INDEX, NATIVE_FUNDING_ROW_COLUMNS, NativeFundingFeatureError,
    NativeFundingGazetteerCoverage, NativeFundingGazetteerLane, NativeFundingGazetteerSpan,
    check_funding_feature_columns, render_funding_feature_text, render_funding_features,
    span_covers as funding_span_covers, validate_funding_gazetteer_coverage,
};
pub use native_funding_sequence::{
    MAX_NATIVE_FUNDING_ROWS, NATIVE_FUNDING_BASE_LABELS, NATIVE_FUNDING_LABELS,
    NATIVE_FUNDING_SEQUENCE_SCHEMA_VERSION, NATIVE_FUNDING_UNMODELED_JAVA_LABELS,
    NativeFundingAbsence, NativeFundingDecodeOutcome, NativeFundingEntry, NativeFundingRole,
    NativeFundingSequenceError, check_funding_column_window, check_funding_label_block,
    decode_funding_sequence, funding_entries_from_decoded, funding_label_is_admissible,
    funding_role_of_label,
};
pub use native_header_features::{
    BlockStatus, HeaderAlignment, HeaderFeatureRow, HeaderFeatureToken, HeaderFontStatistics,
    LineStatus, NATIVE_HEADER_FEATURE_COLUMNS, NATIVE_HEADER_FEATURE_SCHEMA_VERSION,
    NativeHeaderFeatureError, render_header_feature_text, render_header_features,
};
pub use native_header_fields::{
    MAX_NATIVE_HEADER_FIELD_CHARS, MAX_NATIVE_HEADER_FIELD_VALUES,
    MAX_NATIVE_HEADER_PROJECTION_ROWS, NATIVE_HEADER_FIELDS_SCHEMA_VERSION, NativeHeaderFields,
    NativeHeaderFieldsError, project_native_header_fields,
};
pub use native_header_native_input::{
    HeaderAtom, HeaderAtomDisposition, HeaderCoverageVerdict, HeaderOracleIndependenceError,
    HeaderOracleProvenance, NATIVE_HEADER_INDENT_EPSILON_PT, NATIVE_HEADER_INPUT_GENERATOR_ID,
    NATIVE_HEADER_INPUT_SCHEMA_VERSION, NativeHeaderAtomOutcome, NativeHeaderCrfScope,
    NativeHeaderFromInputOutcome, NativeHeaderInputError, NativeHeaderInputIdentity,
    NativeHeaderOutOfScope, build_header_feature_tokens, build_native_header_input,
    classify_header_coverage, derive_header_font_statistics, header_char_set,
    normalize_header_text, require_independent_header_oracle,
};
pub use native_header_sequence::{
    HeaderFieldSpan, MAX_NATIVE_HEADER_SEQUENCE_ROWS, NATIVE_HEADER_LABELS,
    NATIVE_HEADER_SEQUENCE_SCHEMA_VERSION, NativeHeaderDecodeCandidate, NativeHeaderDecodeError,
    NativeHeaderDecodeOutcome, decode_header_sequence,
};
pub use native_heading_regions::{
    HEADING_REGION_ORDER, HeadingEvidence, HeadingMatch, HeadingRegionConfig,
    MAX_NATIVE_VISUAL_LINES, NATIVE_HEADING_REGIONS_SCHEMA_VERSION, NativeHeadingAbsence,
    NativeHeadingAssignment, NativeHeadingIdentity, NativeHeadingRegionError,
    NativeHeadingRegionOutcome, NativeVisualLine, assign_heading_regions,
};
pub use native_personal_name::{
    NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP, NATIVE_PERSONAL_NAME_OUTCOME_SCHEMA_VERSION,
    NATIVE_PERSONAL_NAME_PARSER_ID, NATIVE_PERSONAL_NAME_PARSER_VERSION,
    NativePersonalNameAmbiguous, NativePersonalNameMalformed, NativePersonalNameOutcome,
    NativePersonalNameParser, NativePersonalNameUnsupported, PersonalNameContext,
    RecognizedPersonalName,
};
pub use native_region_input::{
    MAX_NATIVE_REGION_TOKENS, NATIVE_REGION_INPUT_SCHEMA_VERSION, NativeRegionAbsence,
    NativeRegionCarrier, NativeRegionError, NativeRegionIdentity, NativeRegionRun,
    NativeRegionToken, build_native_region_carrier, build_native_word_region_carrier,
};
pub use native_segmentation::{
    DocumentRegion, FULL_SEGMENTATION_REGION_COUNT, FULL_SEGMENTATION_SCHEMA_VERSION,
    FullFurnitureAnnotation, FullFurnitureSubtype, FullLatticeSegmenter, FullLocation,
    FullRegionAbsence, FullRegionDisposition, FullRegionRun, FullSegmentationError,
    FullSegmentationIdentity, FullSegmentationOutcome, FullSemanticRegion, FullTokenAssignment,
    LexemeRow, MAX_LEXEME_BYTES, MAX_PAGES as NATIVE_SEGMENTATION_MAX_PAGES,
    MAX_REGIONS as NATIVE_SEGMENTATION_MAX_REGIONS, MAX_SECTION_DEPTH,
    MAX_SECTIONS as NATIVE_SEGMENTATION_MAX_SECTIONS, MAX_TOKENS as NATIVE_SEGMENTATION_MAX_TOKENS,
    NATIVE_SEGMENTATION_SCHEMA_VERSION, NativeSegmenter, SectionReceipt, SegmentationConfig,
    SegmentationError, SegmentationLimits, SegmentationOutcome, TokenAssignment, segment_document,
    segment_document_full,
};
pub use native_table_sequence::{
    MAX_NATIVE_TABLE_ROWS, NATIVE_TABLE_BASE_LABELS, NATIVE_TABLE_EMITTED_COLUMNS,
    NATIVE_TABLE_LABELS, NATIVE_TABLE_MODEL_MAX_COLUMN_INDEX, NATIVE_TABLE_SEQUENCE_SCHEMA_VERSION,
    NativeTableAbsence, NativeTableCell, NativeTableCells, NativeTableDecodeOutcome,
    NativeTableRevertReason, NativeTableSequenceError, NativeTableSpan, check_table_column_window,
    check_table_label_block, cluster_opens_new_table, decode_table_sequence,
    table_cells_from_inventory, table_label_is_admissible, table_revert_reason,
    table_spans_from_decoded,
};
pub use native_visual::{
    FigureHeadReceipt, FormulaHeadReceipt, MAX_DIGEST_BYTES, MAX_PRIMITIVES, MAX_RECEIPTS,
    NATIVE_VISUAL_SCHEMA_VERSION, NATIVE_VISUAL_SEQUENCE_SCHEMA_VERSION,
    NATIVE_VISUAL_TYPED_ABSENCE_ADR_REF, NATIVE_VISUAL_TYPED_ABSENCE_CONTRACT_ID,
    NATIVE_VISUAL_TYPED_ABSENCE_DECLARED_DENOMINATOR,
    NATIVE_VISUAL_TYPED_ABSENCE_DENOMINATOR_FIELD, NATIVE_VISUAL_TYPED_ABSENCE_EMPTY_POPULATION,
    NATIVE_VISUAL_TYPED_ABSENCE_MISSING_DENOMINATOR,
    NATIVE_VISUAL_TYPED_ABSENCE_OBSERVED_POPULATION, NATIVE_VISUAL_TYPED_ABSENCE_POOLING,
    NATIVE_VISUAL_TYPED_ABSENCE_REASON_FIELD, NATIVE_VISUAL_TYPED_ABSENCE_TERMINAL_IS_PASS,
    NATIVE_VISUAL_TYPED_ABSENCE_UNITY_DENOMINATOR, NATIVE_VISUAL_TYPED_ABSENCE_UNTYPED_EMPTY_SHAPE,
    NATIVE_VISUAL_TYPED_ABSENCE_USED_BY, NATIVE_VISUAL_TYPED_ABSENCE_VOCABULARY,
    NativeTableCellReceipt, NativeVisualAbsence, NativeVisualConfig, NativeVisualDigestReceipt,
    NativeVisualError, NativeVisualHead, NativeVisualHeadOutcome, NativeVisualJavaFallback,
    NativeVisualKind, NativeVisualMetrics, NativeVisualObservation, NativeVisualPrimitive,
    NativeVisualProvider, NativeVisualSequenceIdentity, NativeVisualSequenceReceipt,
    NativeVisualSequenceStep, TYPED_ABSENCE_TAXONOMY, TableHeadReceipt, detect_figure_heads,
    detect_formula_heads, detect_table_heads, run_visual_heads_sequence,
};
pub use native_visual_family_cluster::{
    FamilyLabelCluster, MAX_NATIVE_VISUAL_FAMILY_ROWS, NATIVE_VISUAL_FAMILY_NAME_PREFIX,
    NativeCaptionLinkAbsence, NativeCaptionLinks, NativeCaptionMarker,
    NativeVisualFamilyClusterError, caption_links_from_markers, cluster_family_label_runs,
};
pub use native_word_projection::{
    MAX_NATIVE_WORD_PARENTS, MAX_NATIVE_WORDS, NATIVE_WORD_PROJECTION_SCHEMA_VERSION,
    NativeWordAbsence, NativeWordGeometry, NativeWordGeometryOrigin, NativeWordIdentity,
    NativeWordParent, NativeWordProjection, NativeWordProjectionError, NativeWordToken,
    native_word_keys, native_word_order_key, native_word_reconstruction_digest,
    project_native_words,
};
pub use p5_shadow_join::{
    JavaP2Observation, JavaP5Observation, NativeBodyShadowSummary, P5_SHADOW_JOIN_SCHEMA_VERSION,
    P5ShadowJoinError, P5ShadowJoinIdentity, P5ShadowJoinInput, P5ShadowJoinReport, join_p5_shadow,
};
pub use parser::{ScholarlyParser, lift_and_project_envelope};
pub use profile::GrobidProfile;
pub use stage_tape::{
    CaptureState, ModelCoverage, STAGE_TAPE_SCHEMA_VERSION, StageTape, TapeFinding,
    TapeObservation, verify_tape,
};
pub use transport::{
    CitationsConsolidation, Consolidation, CoordinateFamily, CoordinateSet,
    FULLTEXT_REQUEST_SCHEMA_VERSION, FailureCode, FailurePhase, FulltextDialectV1,
    FulltextRequestVersioned, FundersConsolidation, HEADER_REQUEST_SCHEMA_VERSION,
    HeaderConsolidation, HeaderDialectV1, HeaderRequestVersioned, InvalidConsolidation,
    InvalidPageWindow, LimitKind, PARSE_FAILURE_SCHEMA_VERSION, PageWindow, ParseBackendFailure,
    ParseEndpoint, ParseResultKind, REFERENCES_REQUEST_SCHEMA_VERSION, ReferencesDialectV1,
    ReferencesRequestVersioned, RetryAfter,
};

#[cfg(test)]
mod real_tei_tests;
#[cfg(test)]
mod tests;
