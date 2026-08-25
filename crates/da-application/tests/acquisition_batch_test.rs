use std::sync::Arc;

use da_application::acquisition_batch::BatchAcquisitionUseCase;
use da_domain::acquisition_batch::BatchAcquisitionStatus;
use da_ports::MockSourceAcquisition;

#[test]
fn catalog_hits_are_skip_only_and_d127_locked() {
    let port = Arc::new(MockSourceAcquisition::catalog_hit_pdf(
        "2501.00001",
        "catalog/paper.pdf",
        &"a".repeat(64),
    ));
    let use_case = BatchAcquisitionUseCase::new(port);
    let mut progress = Vec::new();
    let report = use_case
        .execute(&["2501.00001".into(), "2501.00002".into()], false, |item| {
            progress.push((item.ordinal, item.id.clone()))
        })
        .unwrap();
    assert_eq!(
        (report.total, report.ok, report.skip, report.fail),
        (2, 0, 2, 0)
    );
    assert_eq!(
        progress,
        vec![(1, "2501.00001".into()), (2, "2501.00002".into())]
    );
    assert!(
        report
            .items
            .iter()
            .all(|item| item.status == BatchAcquisitionStatus::Skip && !item.import_eligible)
    );
    assert!(!report.import_eligible);
    report.validate().unwrap();
}

#[test]
fn network_denied_misses_are_explicit_failures() {
    let use_case =
        BatchAcquisitionUseCase::new(Arc::new(MockSourceAcquisition::miss_requires_network()));
    let report = use_case
        .execute(&["2501.00001".into()], false, |_| {})
        .unwrap();
    assert_eq!((report.ok, report.skip, report.fail), (0, 0, 1));
    assert!(
        report.items[0]
            .error
            .as_deref()
            .unwrap()
            .contains("network denied")
    );
    assert!(!report.import_eligible);
}
