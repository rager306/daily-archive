mod common;

use common::feature_provider_parity::{
    ProviderParityError, compare_providers, demo_abi, extract_provider, fixture_path,
};
use da_domain::FeatureAbiError;
use std::fs;

#[test]
fn same_page_from_two_provider_fixtures_has_identical_feature_vector() {
    let abi = demo_abi();
    let (left, right, vector) = compare_providers(
        &abi,
        &fixture_path("provider_pdfalto_page.json"),
        &fixture_path("provider_pdf_inspector_page.json"),
    )
    .expect("independent provider fixtures should be comparable");

    assert_eq!(left, "pdfalto-fixture");
    assert_eq!(right, "pdf-inspector-fixture");
    // Independent expected data: [Heading, water], four ABI inputs per token.
    assert_eq!(vector, vec![1.0, 10.0, 10.0, 1.0, 0.0, 10.0, 20.0, 0.0]);
}

#[test]
fn missing_fixture_is_fail_closed_without_absolute_path() {
    let missing = fixture_path("does-not-exist.json");
    let error = extract_provider(&demo_abi(), &missing).unwrap_err();
    assert!(matches!(error, ProviderParityError::MissingFixture { .. }));
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("does-not-exist.json"));
    assert!(!diagnostic.contains("/root/"));
}

#[test]
fn malformed_fixture_is_typed_and_does_not_echo_payload() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("corrupt.json");
    let secret_like_payload = "not-a-provider-payload";
    fs::write(&path, format!("{{\"provider\":\"{secret_like_payload}")).unwrap();

    let error = extract_provider(&demo_abi(), &path).unwrap_err();
    assert!(matches!(
        error,
        ProviderParityError::MalformedFixture { .. }
    ));
    assert!(!error.to_string().contains(secret_like_payload));
    assert!(
        !error
            .to_string()
            .contains(directory.path().to_string_lossy().as_ref())
    );
}

#[test]
fn incompatible_abi_dimension_fails_closed_before_comparison() {
    let mut abi = demo_abi();
    abi.expected_dimension = Some(7);
    let error = extract_provider(&abi, &fixture_path("provider_pdfalto_page.json")).unwrap_err();
    assert!(matches!(
        error,
        ProviderParityError::Abi {
            source: FeatureAbiError::DimensionMismatch {
                expected: 7,
                actual: 8
            },
            ..
        }
    ));
}

#[test]
fn provider_difference_is_explicit_not_silently_accepted() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("different.json");
    fs::write(
        &path,
        r#"{"provider":"altered-fixture","page":1,"tokens":[{"text":"Heading","page":1,"x":11.0,"y":10.0,"width":40.0,"height":10.0,"bold":true,"italic":false},{"text":"water","page":1,"x":10.0,"y":20.0,"width":30.0,"height":8.0,"bold":false,"italic":false}]}"#,
    )
    .unwrap();
    let error = compare_providers(
        &demo_abi(),
        &fixture_path("provider_pdfalto_page.json"),
        &path,
    )
    .unwrap_err();
    assert!(matches!(error, ProviderParityError::OutputMismatch { .. }));
    assert!(error.to_string().contains("pdfalto-fixture"));
    assert!(error.to_string().contains("altered-fixture"));
}
