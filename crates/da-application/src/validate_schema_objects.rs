//! Validate schema-manifest objects against their norm (ADR-068, M451).
//!
//! GENERIC use case: works for any object class bound in path_config
//! (`schema_objects` table). Articles are the first class; the engine
//! (da-domain::schema_invariants) is driven entirely by the object
//! schema's machine-readable invariant declarations.

use da_domain::schema_invariants::{
    validate_file_backed, validate_object, ObjectNorm, SchemaViolation,
};
use da_ports::schema_objects::{SchemaObjectStore, SchemaObjectStoreError};

/// Result of validating a set of objects.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SchemaObjectsValidationReport {
    pub object_class: String,
    pub checked: usize,
    pub violations: Vec<SchemaViolation>,
}

impl SchemaObjectsValidationReport {
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Validate all objects of the store's class (or an explicit subset).
///
/// Violations are reported per object; unreadable objects surface as
/// `UnknownPattern`-kind engine findings? No — store errors abort the
/// run loudly (a broken store is infrastructure, not a data finding).
pub fn validate_schema_objects(
    store: &dyn SchemaObjectStore,
    only: Option<&[String]>,
) -> Result<SchemaObjectsValidationReport, SchemaObjectStoreError> {
    let norm: ObjectNorm = store.norm()?;
    let ids: Vec<String> = match only {
        Some(wanted) => {
            // requested-but-absent ids must be loud
            let known = store.list_ids()?;
            for w in wanted {
                if !known.iter().any(|k| k == w) {
                    return Err(SchemaObjectStoreError::Io(format!(
                        "object {w:?} not found under {} (class {:?})",
                        store.object_class(),
                        store.object_class()
                    )));
                }
            }
            wanted.to_vec()
        }
        None => store.list_ids()?,
    };

    let mut report = SchemaObjectsValidationReport {
        object_class: store.object_class().to_string(),
        checked: 0,
        violations: Vec::new(),
    };

    for id in &ids {
        let Some(object) = store.read_object(id)? else {
            continue;
        };
        report.checked += 1;
        report.violations.extend(validate_object(id, &object, &norm));
        // file-backed patterns through the bridge in the adapter layer:
        // the use case cannot construct it (no dependency on adapters),
        // so expose it via the store itself.
        let io = store.file_backed_checks();
        report
            .violations
            .extend(validate_file_backed(id, &object, &norm, io.as_ref()));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use da_domain::schema_invariants::FileBackedChecks;
    use da_domain::schema_invariants::InvariantDeclaration;
    use std::collections::BTreeMap;

    const NORM_YAML: &str = "lifecycle_order: [raw, parsed]\nlayers:\n  - {id: parsed}\ninvariants:\n  - pattern: derived_state_from_events\n    state_field: lifecycle\n    events_field: lifecycle_events\n    stage_field: stage\n    default_state: raw\n  - pattern: monotonic_timestamp\n    field: lifecycle_events[].recorded_at\n";

    struct FakeStore {
        objects: BTreeMap<String, serde_yaml::Value>,
    }

    impl SchemaObjectStore for FakeStore {
        fn object_class(&self) -> &str {
            "fake"
        }
        fn norm(&self) -> Result<ObjectNorm, SchemaObjectStoreError> {
            ObjectNorm::from_yaml_str(NORM_YAML)
                .map_err(|e| SchemaObjectStoreError::Norm(e.to_string()))
        }
        fn list_ids(&self) -> Result<Vec<String>, SchemaObjectStoreError> {
            Ok(self.objects.keys().cloned().collect())
        }
        fn read_object(
            &self,
            id: &str,
        ) -> Result<Option<serde_yaml::Value>, SchemaObjectStoreError> {
            Ok(self.objects.get(id).cloned())
        }
        fn read_collection(
            &self,
            _id: &str,
            _rel: &str,
        ) -> Result<Option<Vec<serde_yaml::Value>>, SchemaObjectStoreError> {
            Ok(None)
        }
        fn file_exists(&self, _id: &str, _rel: &str) -> Result<bool, SchemaObjectStoreError> {
            Ok(true)
        }
        fn read_raw(&self, _id: &str, _rel: &str) -> Result<Option<String>, SchemaObjectStoreError> {
            Ok(None)
        }
        fn file_backed_checks(&self) -> Box<dyn FileBackedChecks + '_> {
            Box::new(NoopChecks)
        }
    }

    struct NoopChecks;
    impl FileBackedChecks for NoopChecks {
        fn collection_records(
            &self,
            _id: &str,
            _decl: &InvariantDeclaration,
        ) -> Result<Option<Vec<serde_yaml::Value>>, String> {
            Ok(None)
        }
        fn artifact_exists(&self, _id: &str, _rel: &str) -> Result<bool, String> {
            Ok(true)
        }
    }


    fn obj(yaml: &str) -> serde_yaml::Value {
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn clean_objects_report_clean() {
        let store = FakeStore {
            objects: BTreeMap::from([
                ("a1".into(), obj("lifecycle: parsed\nlifecycle_events:\n  - {stage: parsed, recorded_at: '1', actor: t}\n")),
            ]),
        };
        let report = validate_schema_objects(&store, None).unwrap();
        assert_eq!(report.checked, 1);
        assert!(report.is_clean());
        assert_eq!(report.object_class, "fake");
    }

    #[test]
    fn hand_set_state_is_reported_per_object() {
        let store = FakeStore {
            objects: BTreeMap::from([
                ("a1".into(), obj("lifecycle: parsed\nlifecycle_events:\n  - {stage: parsed, recorded_at: '1', actor: t}\n")),
                ("bad".into(), obj("lifecycle: validated\nlifecycle_events:\n  - {stage: parsed, recorded_at: '1', actor: t}\n")),
            ]),
        };
        let report = validate_schema_objects(&store, None).unwrap();
        assert_eq!(report.checked, 2);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].object_id, "bad");
    }

    #[test]
    fn only_subset_validated_and_missing_is_loud() {
        let store = FakeStore {
            objects: BTreeMap::from([("a1".into(), obj("lifecycle: raw\nlifecycle_events: []\n"))]),
        };
        let report = validate_schema_objects(&store, Some(&["a1".into()])).unwrap();
        assert_eq!(report.checked, 1);

        let err = validate_schema_objects(&store, Some(&["ghost".into()])).unwrap_err();
        assert!(err.to_string().contains("ghost"));
    }

    #[test]
    fn declaration_params_flow_through() {
        // ensure InvariantDeclaration import is genuinely used (compile guard)
        let d = InvariantDeclaration {
            pattern: "x".into(),
            params: BTreeMap::new(),
        };
        assert!(d.params.is_empty());
    }
}
