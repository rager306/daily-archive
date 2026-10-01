//! S05/T05 contract gate for the native document-group partition and the
//! required-family disposition vocabulary (M514-1lss2s).
//!
//! Default features, fully offline: no `pdf-layout`, no `pdf-classifier`, no
//! filesystem, no service. The vocabulary is `std`/`serde` only.
//!
//! Negative polarity is the point of this file:
//!
//! 1. every closed dictionary is closed — a token outside it is refused;
//! 2. the partition is total and disjoint — a gap, an overlap, an undeclared
//!    extra or an inconsistent row is a typed refusal, never a silent default;
//! 3. a document with absent bytes is *not* `unknown` — the absence carries no
//!    group at all, and an attempt to smuggle one in is refused;
//! 4. a scanned/image-only document forces `unmet{no_text_layer}` for a
//!    required family;
//! 5. `out_of_scope` cannot be a required-family disposition — deserialization
//!    refuses it and `parse` refuses it **by name**;
//! 6. an empty declared corpus is a typed absence, not a zero.

use da_cli::native_document_group::{
    DOCUMENT_BYTES_ABSENT_VOCABULARY, DOCUMENT_GROUP_SOURCE_VOCABULARY, DOCUMENT_GROUP_VOCABULARY,
    DocumentBytesAbsentReason, DocumentGroup, DocumentGroupError, DocumentGroupEvidence,
    DocumentGroupMeasured, DocumentGroupPartition, DocumentGroupPartitionState,
    DocumentGroupSource, FORBIDDEN_REQUIRED_FAMILY_TOKEN, REQUIRED_FAMILY_DISPOSITION_VOCABULARY,
    REQUIRED_FAMILY_UNMET_VOCABULARY, RequiredFamilyDisposition, RequiredFamilyUnmetReason,
};
use da_domain::pdf_classification::PdfType;

fn measured(document_id: &str, pdf_type: PdfType) -> DocumentGroupEvidence {
    DocumentGroupEvidence::measured(
        document_id,
        pdf_type,
        "pdf-inspector",
        "0.1.0",
        format!("sha256:{}", document_id),
    )
}

fn declared(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_string()).collect()
}

#[test]
fn document_group_dictionary_is_closed_and_in_lockstep() {
    assert_eq!(DOCUMENT_GROUP_VOCABULARY.len(), 4);
    let arms = [
        DocumentGroup::DigitalText,
        DocumentGroup::ScannedImage,
        DocumentGroup::Mixed,
        DocumentGroup::Unknown,
    ];
    for (arm, token) in arms.iter().zip(DOCUMENT_GROUP_VOCABULARY) {
        assert_eq!(arm.token(), token);
        assert_eq!(DocumentGroup::from_token(token).unwrap(), *arm);
        assert_eq!(
            serde_json::to_value(arm).unwrap(),
            serde_json::Value::String(token.to_string())
        );
        let back: DocumentGroup = serde_json::from_str(&format!("\"{token}\"")).unwrap();
        assert_eq!(back, *arm);
    }
    assert!(!DOCUMENT_GROUP_VOCABULARY.contains(&FORBIDDEN_REQUIRED_FAMILY_TOKEN));

    // Closed: an outside token is refused by the parser and by serde.
    assert!(matches!(
        DocumentGroup::from_token("out_of_scope"),
        Err(DocumentGroupError::UnknownVocabularyToken {
            vocabulary: "document_group",
            ..
        })
    ));
    assert!(serde_json::from_str::<DocumentGroup>("\"out_of_scope\"").is_err());
    assert!(serde_json::from_str::<DocumentGroup>("\"digital\"").is_err());

    // The other closed dictionaries are closed too.
    assert_eq!(
        DOCUMENT_GROUP_SOURCE_VOCABULARY,
        ["measured", "bytes_absent"]
    );
    assert!(DocumentGroup::from_token("").is_err());
    for (arm, token) in [
        DocumentBytesAbsentReason::PdfBytesMissing,
        DocumentBytesAbsentReason::PdfReadFailed,
        DocumentBytesAbsentReason::ClassifierUnavailable,
    ]
    .iter()
    .zip(DOCUMENT_BYTES_ABSENT_VOCABULARY)
    {
        assert_eq!(arm.token(), token);
        assert_eq!(DocumentBytesAbsentReason::from_token(token).unwrap(), *arm);
    }
    assert!(DocumentBytesAbsentReason::from_token("out_of_scope").is_err());
}

#[test]
fn pdf_type_mapping_is_total_and_single_source() {
    // All five domain values are mapped, and `Scanned`/`Image` deliberately
    // share the no-text-layer arm.
    let cases = [
        (PdfType::Text, DocumentGroup::DigitalText),
        (PdfType::Scanned, DocumentGroup::ScannedImage),
        (PdfType::Image, DocumentGroup::ScannedImage),
        (PdfType::Mixed, DocumentGroup::Mixed),
        (PdfType::Unknown, DocumentGroup::Unknown),
    ];
    for (pdf_type, expected) in &cases {
        assert_eq!(
            DocumentGroup::from_pdf_type(pdf_type),
            *expected,
            "{pdf_type:?}"
        );
    }

    // The mapping is surjective: every arm is reachable, so no arm is dead.
    let reachable: std::collections::BTreeSet<DocumentGroup> = cases
        .iter()
        .map(|(pdf_type, _)| DocumentGroup::from_pdf_type(pdf_type))
        .collect();
    let all: std::collections::BTreeSet<DocumentGroup> = DOCUMENT_GROUP_VOCABULARY
        .iter()
        .map(|token| DocumentGroup::from_token(token).unwrap())
        .collect();
    assert_eq!(reachable, all);

    // A measured row derives its group through the same single mapping.
    let evidence = measured("hub", PdfType::Scanned);
    assert_eq!(evidence.assigned_group(), Some(DocumentGroup::ScannedImage));
    assert_eq!(
        evidence.forced_required_family_disposition(),
        Some(RequiredFamilyDisposition::Unmet(
            RequiredFamilyUnmetReason::NoTextLayer
        ))
    );
}

#[test]
fn partition_is_total_and_disjoint() {
    let partition = DocumentGroupPartition::assemble(
        "layout_bakeoff",
        &declared(&["hub", "rlm", "reca"]),
        vec![
            measured("hub", PdfType::Text),
            measured("rlm", PdfType::Scanned),
            measured("reca", PdfType::Mixed),
        ],
    )
    .expect("total partition");

    assert_eq!(partition.state, DocumentGroupPartitionState::Partitioned);
    assert_eq!(partition.declared_document_count, 3);
    assert_eq!(partition.measured_count(), 3);
    assert_eq!(partition.bytes_absent_count(), 0);
    assert_eq!(partition.group_of("hub"), Some(DocumentGroup::DigitalText));
    assert_eq!(partition.group_of("rlm"), Some(DocumentGroup::ScannedImage));
    assert_eq!(partition.group_of("reca"), Some(DocumentGroup::Mixed));
    assert_eq!(
        partition.groups_present(),
        vec![
            DocumentGroup::DigitalText,
            DocumentGroup::ScannedImage,
            DocumentGroup::Mixed,
        ]
    );
    assert_eq!(partition.group_counts().len(), 3);

    // A gap is refused, not filled with a default.
    assert!(matches!(
        DocumentGroupPartition::assemble("c", &declared(&["hub", "rlm"]), vec![measured("hub", PdfType::Text)]),
        Err(DocumentGroupError::MissingDocumentAssignment { document_id }) if document_id == "rlm"
    ));
    // An overlap is refused.
    assert!(matches!(
        DocumentGroupPartition::assemble(
            "c",
            &declared(&["hub", "rlm"]),
            vec![measured("hub", PdfType::Text), measured("hub", PdfType::Text), measured("rlm", PdfType::Text)]
        ),
        Err(DocumentGroupError::DuplicateDocumentAssignment { document_id }) if document_id == "hub"
    ));
    // An undeclared extra is refused.
    assert!(matches!(
        DocumentGroupPartition::assemble(
            "c",
            &declared(&["hub", "rlm"]),
            vec![measured("hub", PdfType::Text), measured("rlm", PdfType::Text), measured("ghost", PdfType::Text)]
        ),
        Err(DocumentGroupError::UndeclaredDocumentAssignment { document_id }) if document_id == "ghost"
    ));
    // A duplicated declaration is refused.
    assert!(matches!(
        DocumentGroupPartition::assemble("c", &declared(&["hub", "hub"]), vec![measured("hub", PdfType::Text)]),
        Err(DocumentGroupError::DuplicateDeclaredDocument { document_id }) if document_id == "hub"
    ));
    // A blank corpus id or document id is refused.
    assert!(matches!(
        DocumentGroupPartition::assemble(
            "",
            &declared(&["hub"]),
            vec![measured("hub", PdfType::Text)]
        ),
        Err(DocumentGroupError::EmptyCorpusId)
    ));
    assert!(matches!(
        measured("", PdfType::Text).validate(),
        Err(DocumentGroupError::EmptyDocumentId)
    ));
}

#[test]
fn bytes_absent_is_a_typed_absence_never_unknown() {
    let partition = DocumentGroupPartition::assemble(
        "c",
        &declared(&["hub", "rlm"]),
        vec![
            measured("hub", PdfType::Text),
            DocumentGroupEvidence::bytes_absent("rlm", DocumentBytesAbsentReason::PdfReadFailed),
        ],
    )
    .expect("partition with a typed absence");

    assert_eq!(partition.group_of("rlm"), None);
    assert_eq!(partition.bytes_absent_count(), 1);
    assert_eq!(partition.measured_count(), 1);
    assert!(
        !partition
            .group_counts()
            .contains_key(&DocumentGroup::Unknown)
    );
    assert_eq!(partition.groups_present(), vec![DocumentGroup::DigitalText]);
    assert_eq!(partition.forced_required_family_disposition("rlm"), None);
    assert_eq!(
        partition
            .evidence_for("rlm")
            .and_then(DocumentGroupEvidence::bytes_absent_reason),
        Some(DocumentBytesAbsentReason::PdfReadFailed)
    );

    // Serialized absence carries no `group` key at all.
    let value = serde_json::to_value(DocumentGroupEvidence::bytes_absent(
        "rlm",
        DocumentBytesAbsentReason::PdfBytesMissing,
    ))
    .unwrap();
    assert!(value.get("group").is_none());
    assert_eq!(value["source"]["kind"], "bytes_absent");

    // Smuggling a group onto an absence is refused by validation.
    let smuggled = DocumentGroupEvidence {
        document_id: "rlm".into(),
        group: Some(DocumentGroup::Unknown),
        source: DocumentGroupSource::BytesAbsent(
            da_cli::native_document_group::DocumentGroupBytesAbsent {
                reason: DocumentBytesAbsentReason::PdfBytesMissing,
            },
        ),
    };
    assert!(matches!(
        smuggled.validate(),
        Err(DocumentGroupError::BytesAbsentCarriesGroup { document_id }) if document_id == "rlm"
    ));

    // And a measured row without a group is refused: the two facts stay apart.
    let headless = DocumentGroupEvidence {
        document_id: "rlm".into(),
        group: None,
        source: DocumentGroupSource::Measured(DocumentGroupMeasured {
            pdf_type: PdfType::Text,
            classifier_id: "pdf-inspector".into(),
            classifier_version: "0.1.0".into(),
            pdf_sha256: "sha256:rlm".into(),
        }),
    };
    assert!(matches!(
        headless.validate(),
        Err(DocumentGroupError::MeasuredWithoutGroup { document_id }) if document_id == "rlm"
    ));
}

#[test]
fn scanned_group_forces_unmet_no_text_layer() {
    assert_eq!(
        DocumentGroup::ScannedImage.text_layer_absence(),
        Some(RequiredFamilyUnmetReason::NoTextLayer)
    );
    assert_eq!(
        DocumentGroup::ScannedImage.forced_required_family_disposition(),
        Some(RequiredFamilyDisposition::Unmet(
            RequiredFamilyUnmetReason::NoTextLayer
        ))
    );
    for group in [
        DocumentGroup::DigitalText,
        DocumentGroup::Mixed,
        DocumentGroup::Unknown,
    ] {
        assert_eq!(group.text_layer_absence(), None, "{group:?}");
        assert_eq!(
            group.forced_required_family_disposition(),
            None,
            "{group:?}"
        );
    }

    // Image-only documents are folded onto the same arm, so they force the same
    // unmet reason rather than a false measurement.
    let image_only = DocumentGroupPartition::assemble(
        "c",
        &declared(&["img"]),
        vec![measured("img", PdfType::Image)],
    )
    .unwrap();
    assert_eq!(
        image_only.forced_required_family_disposition("img"),
        Some(RequiredFamilyDisposition::Unmet(
            RequiredFamilyUnmetReason::NoTextLayer
        ))
    );
}

#[test]
fn measured_group_must_match_its_own_pdf_type() {
    let inconsistent = DocumentGroupEvidence {
        document_id: "hub".into(),
        group: Some(DocumentGroup::DigitalText),
        source: DocumentGroupSource::Measured(DocumentGroupMeasured {
            pdf_type: PdfType::Scanned,
            classifier_id: "pdf-inspector".into(),
            classifier_version: "0.1.0".into(),
            pdf_sha256: "sha256:hub".into(),
        }),
    };
    assert!(matches!(
        inconsistent.validate(),
        Err(DocumentGroupError::GroupPdfTypeMismatch {
            group: DocumentGroup::DigitalText,
            ..
        })
    ));

    let blank = DocumentGroupEvidence::measured("hub", PdfType::Text, "", "0.1.0", "sha256:hub");
    assert!(matches!(
        blank.validate(),
        Err(DocumentGroupError::EmptyMeasuredField {
            field: "classifier_id",
            ..
        })
    ));
}

#[test]
fn out_of_scope_is_structurally_absent_from_the_required_family_disposition() {
    assert!(!REQUIRED_FAMILY_DISPOSITION_VOCABULARY.contains(&FORBIDDEN_REQUIRED_FAMILY_TOKEN));
    assert!(!REQUIRED_FAMILY_UNMET_VOCABULARY.contains(&FORBIDDEN_REQUIRED_FAMILY_TOKEN));

    // The closed dictionary is exactly three arms, none of them out of scope.
    assert_eq!(
        REQUIRED_FAMILY_DISPOSITION_VOCABULARY,
        ["measured", "unmet", "not_observed"]
    );

    // Deserialization is fail-closed: the forbidden token has no variant.
    assert!(serde_json::from_str::<RequiredFamilyDisposition>("\"out_of_scope\"").is_err());
    assert!(serde_json::from_str::<RequiredFamilyUnmetReason>("\"out_of_scope\"").is_err());

    // The parser refuses the token BY NAME, so a validator can label the
    // mutation instead of reporting a generic parse error.
    assert_eq!(
        RequiredFamilyDisposition::parse("out_of_scope"),
        Err(DocumentGroupError::RequiredFamilyOutOfScope {
            token: "out_of_scope".to_string()
        })
    );
    assert!(matches!(
        RequiredFamilyUnmetReason::from_token("out_of_scope"),
        Err(DocumentGroupError::UnknownVocabularyToken { .. })
    ));

    // And no arm of the disposition ever serializes to the forbidden token.
    for disposition in [
        RequiredFamilyDisposition::Measured,
        RequiredFamilyDisposition::NotObserved,
        RequiredFamilyDisposition::Unmet(RequiredFamilyUnmetReason::NoTextLayer),
        RequiredFamilyDisposition::Unmet(RequiredFamilyUnmetReason::NativeLaneUnsupported),
        RequiredFamilyDisposition::Unmet(RequiredFamilyUnmetReason::OracleAbsent),
        RequiredFamilyDisposition::Unmet(RequiredFamilyUnmetReason::OracleVacuous),
    ] {
        let value = serde_json::to_value(disposition).unwrap();
        assert_ne!(
            value,
            serde_json::Value::String(FORBIDDEN_REQUIRED_FAMILY_TOKEN.to_string())
        );
        assert!(
            !value.to_string().contains("out_of_scope"),
            "{disposition:?} leaked the forbidden token: {value}"
        );
    }
    // `unmet` without a reason is refused, because it is an untyped hole.
    assert_eq!(
        RequiredFamilyDisposition::parse("unmet"),
        Err(DocumentGroupError::UnmetRequiresReason)
    );
}

#[test]
fn required_family_disposition_vocabulary_is_closed_and_roundtrips() {
    for reason in [
        RequiredFamilyUnmetReason::NoTextLayer,
        RequiredFamilyUnmetReason::NativeLaneUnsupported,
        RequiredFamilyUnmetReason::OracleAbsent,
        RequiredFamilyUnmetReason::OracleVacuous,
    ] {
        assert!(REQUIRED_FAMILY_UNMET_VOCABULARY.contains(&reason.token()));
        assert_eq!(
            RequiredFamilyUnmetReason::from_token(reason.token()),
            Ok(reason)
        );
        let disposition = RequiredFamilyDisposition::unmet(reason);
        assert_eq!(disposition.unmet_reason(), Some(reason));
        assert!(!disposition.is_measured());
        let value = serde_json::to_value(disposition).unwrap();
        let back: RequiredFamilyDisposition = serde_json::from_value(value).unwrap();
        assert_eq!(back, disposition);
    }

    for unit in [
        RequiredFamilyDisposition::Measured,
        RequiredFamilyDisposition::NotObserved,
    ] {
        assert!(REQUIRED_FAMILY_DISPOSITION_VOCABULARY.contains(&unit.token()));
        assert_eq!(RequiredFamilyDisposition::parse(unit.token()), Ok(unit));
        let value = serde_json::to_value(unit).unwrap();
        let back: RequiredFamilyDisposition = serde_json::from_value(value).unwrap();
        assert_eq!(back, unit);
    }
    assert!(RequiredFamilyDisposition::Measured.is_measured());
    assert!(RequiredFamilyDisposition::Measured.unmet_reason().is_none());
    assert!(REQUIRED_FAMILY_DISPOSITION_VOCABULARY.contains(&"unmet"));
}

#[test]
fn empty_declared_corpus_is_a_typed_absence() {
    let partition = DocumentGroupPartition::assemble("c", &[], vec![]).unwrap();
    assert!(partition.is_empty_declared_corpus());
    assert_eq!(
        partition.state,
        DocumentGroupPartitionState::EmptyDeclaredCorpus
    );
    assert_eq!(partition.declared_document_count, 0);
    assert_eq!(partition.measured_count(), 0);
    assert_eq!(partition.bytes_absent_count(), 0);
    assert!(partition.group_counts().is_empty());
    assert!(partition.groups_present().is_empty());
    assert_eq!(partition.group_of("hub"), None);
    assert_eq!(partition.evidence_for("hub"), None);

    // Assignments cannot be invented for an empty declaration.
    assert!(matches!(
        DocumentGroupPartition::assemble("c", &[], vec![measured("hub", PdfType::Text)]),
        Err(DocumentGroupError::AssignmentsForEmptyDeclaredCorpus { count: 1 })
    ));
}

#[test]
fn partition_roundtrips_byte_for_byte_through_json() {
    let partition = DocumentGroupPartition::assemble(
        "layout_bakeoff",
        &declared(&["hub", "rlm", "reca"]),
        vec![
            measured("hub", PdfType::Text),
            DocumentGroupEvidence::bytes_absent(
                "rlm",
                DocumentBytesAbsentReason::ClassifierUnavailable,
            ),
            measured("reca", PdfType::Mixed),
        ],
    )
    .unwrap();
    let json = serde_json::to_string(&partition).unwrap();
    let back: DocumentGroupPartition = serde_json::from_str(&json).unwrap();
    assert_eq!(partition, back);
    assert_eq!(serde_json::to_string(&back).unwrap(), json);

    // The measured arm carries both the raw verdict and its mapped group.
    let value = serde_json::to_value(measured("hub", PdfType::Text)).unwrap();
    assert_eq!(value["group"], "digital_text");
    assert_eq!(value["source"]["kind"], "measured");
    assert_eq!(value["source"]["pdf_type"], "text");
    assert_eq!(value["source"]["classifier_id"], "pdf-inspector");
    assert_eq!(value["source"]["classifier_version"], "0.1.0");
}
