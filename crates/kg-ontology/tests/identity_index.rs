use kg_ontology::{IdentityIndexPolicy, OntologyRegistry, PropertyIndexPolicy};

fn registry() -> OntologyRegistry {
    OntologyRegistry::load_from_str(
        r#"
node_schemas:
  - label: Paper
    required_fields:
      - {name: vid, field_type: String}
      - {name: arxiv_id, field_type: String}
  - label: Section
    required_fields:
      - {name: vid, field_type: String}
"#,
        "nodes.yaml",
    )
    .unwrap()
}

#[test]
fn policy_is_validated_sorted_and_fingerprinted_deterministically() {
    let yaml = r#"
schema_version: ontology-v2-identity-indexes.v1
indexes:
  - {label: Section, property: vid, identity: true}
  - {label: Paper, property: arxiv_id, identity: true}
  - {label: Paper, property: vid, identity: true}
"#;
    let first = IdentityIndexPolicy::parse_and_validate(yaml.as_bytes(), &registry()).unwrap();
    let second = IdentityIndexPolicy::parse_and_validate(yaml.as_bytes(), &registry()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.indexes()[0].key(), "Paper.arxiv_id");
    assert_eq!(first.indexes()[1].key(), "Paper.vid");
    assert_eq!(first.indexes()[2].key(), "Section.vid");
    assert!(first.indexes().iter().all(|index| index.required));
    assert!(first.sha256().starts_with("sha256:"));
    assert_eq!(first.sha256().len(), 71);
}

#[test]
fn optional_natural_identity_is_explicit_and_fingerprinted() {
    let policy = IdentityIndexPolicy::parse_and_validate(
        br#"schema_version: ontology-v2-identity-indexes.v1
indexes: [{label: Paper, property: arxiv_id, identity: true, required: false}]"#,
        &registry(),
    )
    .unwrap();
    assert!(!policy.indexes()[0].required);
}

#[test]
fn policy_rejects_unknown_label_property_duplicate_and_non_identity() {
    for yaml in [
        r#"schema_version: ontology-v2-identity-indexes.v1
indexes: [{label: Missing, property: vid, identity: true}]"#,
        r#"schema_version: ontology-v2-identity-indexes.v1
indexes: [{label: Paper, property: missing, identity: true}]"#,
        r#"schema_version: ontology-v2-identity-indexes.v1
indexes:
  - {label: Paper, property: vid, identity: true}
  - {label: Paper, property: vid, identity: true}"#,
        r#"schema_version: ontology-v2-identity-indexes.v1
indexes: [{label: Paper, property: vid, identity: false}]"#,
    ] {
        assert!(IdentityIndexPolicy::parse_and_validate(yaml.as_bytes(), &registry()).is_err());
    }
}

#[test]
fn policy_rejects_empty_and_wrong_schema() {
    for yaml in [
        "schema_version: wrong\nindexes: [{label: Paper, property: vid, identity: true}]",
        "schema_version: ontology-v2-identity-indexes.v1\nindexes: []",
    ] {
        assert!(IdentityIndexPolicy::parse_and_validate(yaml.as_bytes(), &registry()).is_err());
    }
}

#[test]
fn property_policy_is_separate_validated_and_deterministic() {
    let yaml = br#"
schema_version: ontology-v2-property-indexes.v1
indexes:
  - {label: Paper, property: arxiv_id, identity: false}
"#;
    let first = PropertyIndexPolicy::parse_and_validate(yaml, &registry()).unwrap();
    let second = PropertyIndexPolicy::parse_and_validate(yaml, &registry()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.indexes()[0].key(), "Paper.arxiv_id");
    assert_eq!(first.sha256(), second.sha256());
}

#[test]
fn property_policy_rejects_identity_unknown_and_duplicate_entries() {
    for yaml in [
        r#"schema_version: ontology-v2-property-indexes.v1
indexes: [{label: Paper, property: vid, identity: true}]"#,
        r#"schema_version: ontology-v2-property-indexes.v1
indexes: [{label: Paper, property: missing, identity: false}]"#,
        r#"schema_version: ontology-v2-property-indexes.v1
indexes:
  - {label: Paper, property: vid, identity: false}
  - {label: Paper, property: vid, identity: false}"#,
    ] {
        assert!(PropertyIndexPolicy::parse_and_validate(yaml.as_bytes(), &registry()).is_err());
    }
}

#[test]
fn validated_policy_serialization_is_deterministic_but_not_a_trusted_constructor() {
    let policy = IdentityIndexPolicy::parse_and_validate(
        b"schema_version: ontology-v2-identity-indexes.v1\nindexes: [{label: Paper, property: vid, identity: true}]",
        &registry(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&policy).unwrap(),
        serde_json::to_vec(&policy).unwrap()
    );
    let property = PropertyIndexPolicy::parse_and_validate(
        b"schema_version: ontology-v2-property-indexes.v1\nindexes: [{label: Paper, property: arxiv_id, identity: false}]",
        &registry(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&property).unwrap(),
        serde_json::to_vec(&property).unwrap()
    );
}
