//! Donor visual sidecar → native visual observation adapter (shadow-only).
//!
//! This converter used to live inside the feature-gated example target
//! `examples/native_visual_heads_shadow.rs`. Example unit tests are not built
//! or run by `cargo test -p da-cli --tests`, so the path that must preserve
//! donor primitives (GAR-003) was not observable in any executed gate. Moving
//! the converter onto the library surface under the already-declared
//! `pdf-layout` feature puts its tests into the `unittests src/lib.rs` target,
//! which the existing governor step `"da-cli feature-gated example contracts"`
//! already runs (`cargo test -p da-cli --features pdf-layout --tests`). No
//! governor change is required.
//!
//! The module is gated because every type it consumes
//! (`da_adapters::VisualObservation`, `LayoutShadowObservation`,
//! `da_adapters::pdf-layout`) exists only with `pdf-layout`, whose
//! `default = []`. It must stay out of the default feature set.

use da_adapters::{
    LayoutDonorError, LayoutShadowObservation, VisualAbsenceReason, VisualPrimitiveKind,
    VisualPrimitiveState,
};
use da_parse::{
    NativeVisualAbsence, NativeVisualConfig, NativeVisualKind, NativeVisualObservation,
    NativeVisualPrimitive, NativeVisualProvider,
};

/// Receipt families the donor sidecar cannot physically supply.
///
/// `da_adapters::VisualObservation` carries only `primitives` + `absence`, so
/// captions/callouts/cells/labels are never delivered by this donor path. The
/// native contract does have these families (`da_parse::native_visual`), so
/// they are declared explicitly here instead of being silently left empty and
/// mistaken for "the donor supplied nothing".
pub const NON_DELIVERABLE_VISUAL_FAMILIES: [&str; 7] = [
    "figure_captions",
    "figure_callouts",
    "table_captions",
    "table_callouts",
    "table_cells",
    "formula_labels",
    "formula_callouts",
];

/// Identity used when no donor observation can be converted at all.
pub struct VisualIdentity {
    pub document_id: String,
    pub source_sha256: String,
    pub provider_id: String,
    pub provider_version: String,
}

pub fn native_kind(kind: VisualPrimitiveKind) -> NativeVisualKind {
    match kind {
        VisualPrimitiveKind::Image => NativeVisualKind::Image,
        VisualPrimitiveKind::Graphic => NativeVisualKind::Graphic,
        VisualPrimitiveKind::Table => NativeVisualKind::Table,
        VisualPrimitiveKind::Formula => NativeVisualKind::Formula,
        VisualPrimitiveKind::Rect => NativeVisualKind::Rect,
    }
}

/// Convert the donor's bounded visual sidecar without collapsing observed
/// primitives into a terminal empty lane. Receipt families remain explicit:
/// an empty family means that family was not supplied by this donor, while a
/// lane absence is reserved for an actually absent upstream visual signal.
pub fn native_visual_payload(
    observation: &LayoutShadowObservation,
) -> Result<NativeVisualObservation, NativeVisualAbsence> {
    let lane_absence = observation.visual.absence.map(donor_lane_absence);
    let primitives = observation
        .visual
        .primitives
        .iter()
        .map(|primitive| {
            let (bbox, content_digest, absence) = match &primitive.state {
                VisualPrimitiveState::Observed { content_digest } => {
                    (primitive.bbox, Some(content_digest.clone()), None)
                }
                VisualPrimitiveState::Absent { reason } => {
                    (None, None, Some(donor_lane_absence(*reason)))
                }
            };
            NativeVisualPrimitive {
                document_id: primitive.document_id.clone(),
                source_sha256: observation
                    .document
                    .source_pdf_sha256
                    .trim_start_matches("sha256:")
                    .to_owned(),
                provider: NativeVisualProvider {
                    id: primitive.provider.id.clone(),
                    version: primitive.provider.version.clone(),
                },
                page_ordinal: primitive.page_ordinal,
                source_ordinal: primitive.source_ordinal,
                kind: native_kind(primitive.kind),
                bbox,
                content_digest,
                absence,
            }
        })
        .collect::<Vec<_>>();
    let source = NativeVisualObservation {
        schema_version: da_parse::NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: observation.document.document_id.clone(),
        source_sha256: observation
            .document
            .source_pdf_sha256
            .trim_start_matches("sha256:")
            .into(),
        provider: NativeVisualProvider {
            id: observation.visual.provider.id.clone(),
            version: observation.visual.provider.version.clone(),
        },
        primitives,
        figure_captions: Vec::new(),
        figure_callouts: Vec::new(),
        table_captions: Vec::new(),
        table_callouts: Vec::new(),
        table_cells: Vec::new(),
        formula_labels: Vec::new(),
        formula_callouts: Vec::new(),
        lane_absence,
    };
    source
        .validate(NativeVisualConfig::default())
        .map_err(|error| match error {
            da_parse::NativeVisualError::InvalidDigest => NativeVisualAbsence::InvalidDigest,
            da_parse::NativeVisualError::InvalidGeometry
            | da_parse::NativeVisualError::InvalidSchema => NativeVisualAbsence::MalformedBbox,
            da_parse::NativeVisualError::DuplicatePrimitive
            | da_parse::NativeVisualError::DuplicateReceipt => {
                NativeVisualAbsence::DuplicateReceipt
            }
            da_parse::NativeVisualError::Oversize(_) | da_parse::NativeVisualError::HeadLimit => {
                NativeVisualAbsence::Oversize
            }
            da_parse::NativeVisualError::ReceiptMismatch => NativeVisualAbsence::MissingReceipt,
            da_parse::NativeVisualError::InvalidIdentity(_) => NativeVisualAbsence::MissingIdentity,
        })?;
    Ok(source)
}

/// Map a donor lane absence into the native closed taxonomy.
pub fn donor_lane_absence(reason: VisualAbsenceReason) -> NativeVisualAbsence {
    match reason {
        VisualAbsenceReason::Unsupported | VisualAbsenceReason::ProviderUnavailable => {
            NativeVisualAbsence::Unsupported
        }
        VisualAbsenceReason::NotObserved => NativeVisualAbsence::NotObserved,
        VisualAbsenceReason::Malformed => NativeVisualAbsence::MalformedBbox,
    }
}

/// Map a terminal donor error into the native closed taxonomy.
pub fn donor_error_absence(error: &LayoutDonorError) -> NativeVisualAbsence {
    match error {
        LayoutDonorError::Malformed { .. } | LayoutDonorError::InvalidLayout(_) => {
            NativeVisualAbsence::MalformedBbox
        }
        LayoutDonorError::Oversize { .. } => NativeVisualAbsence::Oversize,
        LayoutDonorError::Unavailable { .. } | LayoutDonorError::Io(_) => {
            NativeVisualAbsence::Unsupported
        }
        LayoutDonorError::Timeout { .. }
        | LayoutDonorError::NonZero { .. }
        | LayoutDonorError::Encrypted
        | LayoutDonorError::Truncated => NativeVisualAbsence::Interrupted,
    }
}

/// Canonical empty observation for a failed/absent donor signal.
///
/// All seven non-deliverable receipt families stay empty by construction and
/// the terminal reason is carried in `lane_absence`.
pub fn empty_observation(
    identity: &VisualIdentity,
    lane_absence: NativeVisualAbsence,
) -> NativeVisualObservation {
    NativeVisualObservation {
        schema_version: da_parse::NATIVE_VISUAL_SCHEMA_VERSION.into(),
        document_id: identity.document_id.clone(),
        source_sha256: identity.source_sha256.trim_start_matches("sha256:").into(),
        provider: NativeVisualProvider {
            id: identity.provider_id.clone(),
            version: identity.provider_version.clone(),
        },
        primitives: Vec::new(),
        figure_captions: Vec::new(),
        figure_callouts: Vec::new(),
        table_captions: Vec::new(),
        table_callouts: Vec::new(),
        table_cells: Vec::new(),
        formula_labels: Vec::new(),
        formula_callouts: Vec::new(),
        lane_absence: Some(lane_absence),
    }
}

#[cfg(test)]
mod native_visual_payload_tests {
    use super::*;
    use da_adapters::{LayoutDonorLimits, parse_pdfalto_xml_shadow};
    use da_parse::{NativeVisualHeadOutcome, detect_figure_heads};

    fn payload(xml: &[u8]) -> NativeVisualObservation {
        let shadow = parse_pdfalto_xml_shadow(
            xml,
            b"fixture-pdf",
            "fixture-doc",
            LayoutDonorLimits::default(),
        )
        .expect("positive visual fixture");
        native_visual_payload(&shadow).expect("donor visual payload should be native-valid")
    }

    #[test]
    fn native_visual_payload_preserves_positive_donor_primitive() {
        let source = payload(
            br#"<doc><page width="612" height="792"><Figure x="10" y="20" width="30" height="40" content="figure-a"/></page></doc>"#,
        );
        assert_eq!(source.primitives.len(), 1);
        assert_eq!(source.primitives[0].kind, NativeVisualKind::Image);
        assert!(source.lane_absence.is_none());
        let outcome = detect_figure_heads(&source, NativeVisualConfig::default()).unwrap();
        assert!(matches!(outcome, NativeVisualHeadOutcome::Figure { .. }));
    }

    #[test]
    fn native_visual_payload_is_input_sensitive() {
        let first = payload(
            br#"<doc><page width="612" height="792"><Figure x="10" y="20" width="30" height="40" content="figure-a"/></page></doc>"#,
        );
        let second = payload(
            br#"<doc><page width="612" height="792"><Figure x="100" y="120" width="50" height="60" content="figure-b"/></page></doc>"#,
        );
        assert_ne!(first.primitives, second.primitives);
        let first_outcome = detect_figure_heads(&first, NativeVisualConfig::default()).unwrap();
        let second_outcome = detect_figure_heads(&second, NativeVisualConfig::default()).unwrap();
        assert_ne!(
            serde_json::to_vec(&first_outcome).unwrap(),
            serde_json::to_vec(&second_outcome).unwrap()
        );
    }

    #[test]
    fn malformed_donor_visual_input_is_not_not_observed() {
        let result = parse_pdfalto_xml_shadow(
            br#"<doc><page width="612" height="792"><Figure x="nan" y="20" width="30" height="40" content="bad"/></page></doc>"#,
            b"fixture-pdf",
            "fixture-doc",
            LayoutDonorLimits::default(),
        );
        assert!(matches!(
            result,
            Err(LayoutDonorError::Malformed { .. }) | Err(LayoutDonorError::InvalidLayout(_))
        ));
    }
}
