//! Offline TEI -> IR -> callout-link integration contract (S04/T06).
//!
//! The whole chain is exercised without Java and without network: a preserved
//! GROBID TEI on disk is lifted into this-cut IR, the IR's own body text is
//! scanned for callout markers, and every marker is resolved against the IR's
//! own figure/table/formula/section/citation inventory.
//!
//! The contract asserted here is *link truth*, not counts: a resolved link must
//! name a body element the IR actually declares, and a marker naming a label the
//! body does not declare must stay visible and unresolved. Counts are only
//! asserted as non-empty so the test cannot pass vacuously while staying
//! insensitive to GROBID's evolving output on the frozen fixture.

use da_parse::{
    CalloutBodyInventory, CalloutLabelBasis, CalloutLinkTarget, CalloutLinkUnresolved,
    CalloutMarkerClass, TeiLifter, classify_and_link_callout_markers, classify_callout_markers,
    link_callout_markers,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const FIXTURE: &str = "tests/fixtures/preserved/hub_2507.19457.tei.xml";

fn fixture_ir() -> da_parse::TeiParseIr {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let tei = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("offline TEI fixture unreadable {}: {error}", path.display())
    });
    TeiLifter::lift_tei(tei, "hub-2507.19457", "fixture-pdf-hash")
}

/// Every identity the IR declares — the only bodies a link may point at.
fn declared_ids(ir: &da_parse::TeiParseIr) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for figure in &ir.figures {
        ids.insert(figure.figure_id.clone());
    }
    for table in &ir.tables {
        ids.insert(table.table_id.clone());
    }
    for formula in &ir.formulas {
        ids.insert(formula.formula_id.clone());
    }
    for (position, section) in ir.sections.iter().enumerate() {
        // A section's IR identity is its `path_id` (`5.1`) when the lift bound
        // one; the positional fallback is what the inventory uses only for a
        // section the TEI never numbered.
        ids.insert(
            section
                .path_id
                .clone()
                .unwrap_or_else(|| format!("section-{}", position + 1)),
        );
    }
    for (position, _) in ir.citations.iter().enumerate() {
        ids.insert(format!("bibliography:{}", position + 1));
    }
    ids
}

#[test]
fn callout_markers_link_into_the_real_body() {
    let ir = fixture_ir();
    let report = classify_and_link_callout_markers(&ir).expect("callout scan");

    assert!(
        report.markers.len() >= 20,
        "the frozen body text names fewer markers than expected: {}",
        report.markers.len()
    );
    assert!(report.resolved >= 1, "no marker resolved into the body");
    assert!(report.unresolved >= 1, "no marker stayed unresolved");
    assert!(
        report
            .markers
            .iter()
            .any(|marker| marker.class == CalloutMarkerClass::Figure),
        "the body names no figure at all"
    );

    // The scan is anchored in the real text: every span is a real slice of it.
    for marker in &report.markers {
        assert!(
            ir.body_text.is_char_boundary(marker.span.start)
                && ir.body_text.is_char_boundary(marker.span.end),
            "marker span {} is not a char boundary of the body text",
            marker.index
        );
        assert!(!marker.printed_label.is_empty());
        assert!(!marker.labels.is_empty() || marker.is_range());
    }

    // Anti-fabrication: a resolved link names a declared element, and the
    // resolved/unresolved split reproduces exactly what the inventory claims.
    let inventory = CalloutBodyInventory::from_ir(&ir);
    let declared = declared_ids(&ir);
    for link in &report.links {
        assert_eq!(link.key, format!("{}:{}", link.class.as_str(), link.label));
        let claimed = inventory.contains(link.class, &link.label);
        match &link.target {
            CalloutLinkTarget::Resolved {
                target_id, basis, ..
            } => {
                assert!(
                    declared.contains(target_id),
                    "link resolved to an undeclared element: {target_id}"
                );
                assert!(claimed, "link resolved a label the inventory never claimed");
                assert!(matches!(
                    basis,
                    CalloutLabelBasis::Printed | CalloutLabelBasis::Ordinal
                ));
            }
            CalloutLinkTarget::Unresolved { reason } => {
                if *reason == CalloutLinkUnresolved::Ambiguous {
                    assert!(claimed, "an ambiguous link must be claimed by >1 element");
                } else {
                    assert!(
                        !claimed,
                        "unresolved link {reason:?} names a label the inventory declares"
                    );
                }
            }
        }
    }
}

#[test]
fn callout_markers_are_deterministic_on_the_real_body() {
    let ir = fixture_ir();
    let first = classify_and_link_callout_markers(&ir).expect("first callout scan");
    let second = classify_and_link_callout_markers(&ir).expect("second callout scan");
    assert_eq!(first, second, "the same IR scanned twice disagreed");
    assert_eq!(
        first.schema_version,
        da_parse::CALLOUT_MARKER_SCHEMA_VERSION
    );
}

#[test]
fn callout_markers_never_fabricate_a_target_for_absent_labels() {
    let ir = fixture_ir();
    let inventory = CalloutBodyInventory::from_ir(&ir);
    let markers = classify_callout_markers(
        "Fig. 4000 and Table 4000 and Eq. (4000) and Section 4000.1 and Ref. 40000.",
    )
    .expect("callout scan");
    let links = link_callout_markers(&markers, &inventory);
    assert_eq!(links.len(), markers.len());
    for link in &links {
        assert!(
            !link.is_resolved(),
            "absent label {} fabricated a target",
            link.key
        );
        assert_ne!(
            link.target.reason(),
            Some(CalloutLinkUnresolved::Ambiguous),
            "an absent label cannot be ambiguous"
        );
    }
    // The bodies exist for these classes, so the typed reason is the inventory
    // miss — not "no target at all". This is the distinction the planner asked
    // for: unresolved stays visible and typed.
    assert_eq!(
        links[0].target.reason(),
        Some(CalloutLinkUnresolved::TargetNotInInventory)
    );
}

#[test]
fn callout_markers_find_nothing_in_marker_noise() {
    // Prose about markers is not a marker set: the keyword must name a label.
    for noise in [
        "Figure of speech matters here.",
        "The table of contents lists everything.",
        "Section headings are numbered by the publisher.",
    ] {
        assert!(
            classify_callout_markers(noise)
                .expect("callout scan")
                .is_empty(),
            "noise classified as a marker: {noise}"
        );
    }
}
