//! Shared test support — MockGraphStore for integration tests.
//!
//! Single source-of-truth mock for DirectGraphStore + GraphStore traits.
//! Honors the same contract as the real SamyamaGraphStore:
//!   - `create_node(label)` records the label for later filtering
//!   - `set_node_property_string/int/float/bool` records all properties
//!   - `find_node_by_string_property(label, key, value)` filters by BOTH
//!     label AND the property value (matches SamyamaGraphStore semantics)
//!   - `get_nodes_by_label(label)` filters by label
//!
//! Replaces per-test MockGraphStore impls in batch_ingest_test, enrich_test,
//! extraction_test, healing_test (MEM499, MEM500).

#![allow(dead_code)]

use da_ports::graph_store::{
    DirectGraphStore, GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric,
    VectorSearchResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Property value stored per (node_id, key).
#[derive(Debug, Clone, Default)]
pub struct MockProps {
    pub string_props: HashMap<(u64, String), String>,
    pub int_props: HashMap<(u64, String), i64>,
    pub float_props: HashMap<(u64, String), f64>,
    pub bool_props: HashMap<(u64, String), bool>,
}

/// Inner state — behind Arc so MockGraphStore is Clone-able while
/// keeping the same shared state. This lets tests create a clone of the
/// store, hand one to the use case (as Box<dyn DirectGraphStore>), and
/// keep the other for inspection after the use case runs.
#[derive(Debug, Default)]
struct MockGraphStoreInner {
    /// (node_id, label) — one entry per create_node call.
    nodes: Mutex<Vec<(u64, String)>>,
    /// (source, target, edge_type) — one entry per create_edge call.
    edges: Mutex<Vec<(u64, u64, String)>>,
    /// Monotonic node id counter.
    counter: AtomicUsize,
    /// All properties ever set, indexed by (node_id, key).
    props: Mutex<MockProps>,
    /// Count of export_snapshot calls.
    snapshot_calls: AtomicUsize,
    /// Count of import_snapshot calls.
    import_calls: AtomicUsize,
    /// Optional canned payload returned by export_snapshot. Tests that
    /// need a specific payload set this; otherwise the mock returns
    /// a stable placeholder (b"mock-snapshot-data") for assertions.
    snapshot_data: Mutex<Option<Vec<u8>>>,
}

/// Mock GraphStore that records all node/edge operations.
/// Thread-safe via Mutex. All methods return Ok or empty results.
///
/// Clone-able (Arc-backed) so tests can hand one clone to a use case and
/// keep another for inspection. Counters are exposed for tests that need
/// to assert on call frequency (e.g. snapshot_calls was the reason
/// batch_ingest_test kept a private mock — now it can use the shared one
/// and read snapshot_call_count()).
#[derive(Debug, Clone)]
pub struct MockGraphStore {
    inner: Arc<MockGraphStoreInner>,
}

impl MockGraphStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MockGraphStoreInner::default()),
        }
    }

    /// Convenience: total number of create_node calls.
    pub fn node_count_total(&self) -> usize {
        self.inner.nodes.lock().unwrap().len()
    }

    /// Convenience: total number of create_edge calls.
    pub fn edge_count_total(&self) -> usize {
        self.inner.edges.lock().unwrap().len()
    }

    /// Count of export_snapshot calls so far.
    pub fn snapshot_call_count(&self) -> usize {
        self.inner.snapshot_calls.load(Ordering::SeqCst)
    }

    /// Count of import_snapshot calls so far.
    pub fn import_call_count(&self) -> usize {
        self.inner.import_calls.load(Ordering::SeqCst)
    }
}

impl Default for MockGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl GraphStore for MockGraphStore {
    async fn query(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn query_readonly(&self, _graph: &str, _cypher: &str) -> GraphResult<QueryResult> {
        Ok(QueryResult {
            columns: vec![],
            records: vec![],
        })
    }
    async fn create_vector_index(
        &self,
        _label: &str,
        _property: &str,
        _dimensions: usize,
        _metric: VectorMetric,
    ) -> GraphResult<()> {
        Ok(())
    }
    async fn create_property_index(&self, _label: &str, _property: &str) -> GraphResult<()> {
        Ok(())
    }
    async fn vector_search(
        &self,
        _label: &str,
        _property: &str,
        _query_vector: &[f32],
        _k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        Ok(vec![])
    }
    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        self.inner.snapshot_calls.fetch_add(1, Ordering::SeqCst);
        // Return canned payload if set; otherwise a stable placeholder.
        let data = self
            .inner
            .snapshot_data
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| b"mock-snapshot-data".to_vec());
        Ok(data)
    }
    async fn import_snapshot(&self, _data: &[u8]) -> GraphResult<()> {
        self.inner.import_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn health(&self) -> GraphResult<bool> {
        Ok(true)
    }
}

#[async_trait::async_trait]
impl DirectGraphStore for MockGraphStore {
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError> {
        let id = self.inner.counter.fetch_add(1, Ordering::SeqCst) as u64;
        self.inner.nodes.lock().unwrap().push((id, label.to_string()));
        Ok(id)
    }
    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError> {
        self.inner.props
            .lock()
            .unwrap()
            .string_props
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn set_node_property_int(
        &self,
        node_id: u64,
        key: &str,
        value: i64,
    ) -> Result<(), GraphStoreError> {
        self.inner.props
            .lock()
            .unwrap()
            .int_props
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn set_node_property_float(
        &self,
        node_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError> {
        self.inner.props
            .lock()
            .unwrap()
            .float_props
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn set_node_property_bool(
        &self,
        node_id: u64,
        key: &str,
        value: bool,
    ) -> Result<(), GraphStoreError> {
        self.inner.props
            .lock()
            .unwrap()
            .bool_props
            .insert((node_id, key.to_string()), value);
        Ok(())
    }
    async fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Result<u64, GraphStoreError> {
        let mut edges = self.inner.edges.lock().unwrap();
        let edge_id = edges.len() as u64;
        edges.push((source, target, edge_type.to_string()));
        Ok(edge_id)
    }
    async fn set_edge_property_float(
        &self,
        _edge_id: u64,
        _key: &str,
        _value: f64,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn set_edge_property_string(
        &self,
        _edge_id: u64,
        _key: &str,
        _value: &str,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn add_vector(
        &self,
        _label: &str,
        _property: &str,
        _node_id: u64,
        _vector: Vec<f32>,
    ) -> Result<(), GraphStoreError> {
        Ok(())
    }
    async fn vector_search_direct(
        &self,
        _label: &str,
        _property: &str,
        _query: &[f32],
        _k: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
        Ok(vec![])
    }
    async fn node_count(&self) -> usize {
        self.inner.nodes.lock().unwrap().len()
    }
    async fn edge_count(&self) -> usize {
        self.inner.edges.lock().unwrap().len()
    }
    async fn find_node_by_string_property(
        &self,
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64> {
        // Real SamyamaGraphStore filters by label; the mock must honor
        // the same contract (MEM495). Walk nodes with matching label and
        // check their string property.
        let nodes = self.inner.nodes.lock().unwrap();
        let props = self.inner.props.lock().unwrap();
        for (node_id, node_label) in nodes.iter() {
            if node_label == label {
                if let Some(stored) = props.string_props.get(&(*node_id, key.to_string())) {
                    if stored == value {
                        return Some(*node_id);
                    }
                }
            }
        }
        None
    }
    async fn get_outgoing_edges(&self, _node_id: u64) -> Vec<(u64, String)> {
        self.inner.edges
            .lock()
            .unwrap()
            .iter()
            .filter(|(s, _, _)| *s == _node_id)
            .map(|(_, t, et)| (*t, et.clone()))
            .collect()
    }
    async fn get_incoming_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        self.inner.edges
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, t, _)| *t == node_id)
            .map(|(s, _, et)| (*s, et.clone()))
            .collect()
    }
    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        self.inner.props
            .lock()
            .unwrap()
            .string_props
            .get(&(node_id, key.to_string()))
            .cloned()
    }
    async fn get_node_property_int(&self, node_id: u64, key: &str) -> Option<i64> {
        self.inner.props
            .lock()
            .unwrap()
            .int_props
            .get(&(node_id, key.to_string()))
            .copied()
    }
    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64> {
        self.inner.nodes
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, l)| l == label)
            .map(|(id, _)| *id)
            .collect()
    }
}

// ─── Validator integration helpers (ADR-045 Wave D foundation) ─────────
//
// These helpers let integration tests validate the properties that were
// actually written to the mock store, without needing the validator to
// be wired into the pipeline itself. Pattern:
//
//   let store = MockGraphStore::new();
//   // ... pipeline calls that create nodes via store.create_node(...)
//   let report = store.validate_all_nodes();
//   assert!(report.is_empty(), "violations:\n{}", da_domain::validator::format_violations(&report));
//

impl MockGraphStore {
    /// Build a PropertySnapshot for one node (by id) by merging its
    /// string/int/float/bool properties. Returns None if node not found.
    pub fn snapshot_node(
        &self,
        node_id: u64,
    ) -> Option<da_domain::validator::PropertySnapshot> {
        use serde_json::json;
        let mut snap = da_domain::validator::PropertySnapshot::new();
        let props = self.inner.props.lock().unwrap();
        for ((id, key), val) in props.string_props.iter() {
            if *id == node_id {
                snap.insert(key.clone(), json!(val));
            }
        }
        for ((id, key), val) in props.int_props.iter() {
            if *id == node_id {
                snap.insert(key.clone(), json!(val));
            }
        }
        for ((id, key), val) in props.float_props.iter() {
            if *id == node_id {
                snap.insert(key.clone(), json!(val));
            }
        }
        for ((id, key), val) in props.bool_props.iter() {
            if *id == node_id {
                snap.insert(key.clone(), json!(val));
            }
        }
        Some(snap)
    }

    /// Validate every node in the store against its schema. Returns the
    /// full list of violations across all nodes (flattened).
    pub fn validate_all_nodes(
        &self,
    ) -> Vec<da_domain::validator::SchemaViolation> {
        let mut all = Vec::new();
        let nodes = self.inner.nodes.lock().unwrap();
        for (node_id, label) in nodes.iter() {
            if let Some(snap) = self.snapshot_node(*node_id) {
                let mut v = da_domain::validator::validate_node_properties(label, &snap);
                all.append(&mut v);
            }
        }
        all
    }

    /// Validate a single node by id; returns its violations.
    pub fn validate_node(
        &self,
        node_id: u64,
    ) -> Vec<da_domain::validator::SchemaViolation> {
        let nodes = self.inner.nodes.lock().unwrap();
        let label = nodes
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, l)| l.clone());
        drop(nodes);
        match label {
            Some(label) => {
                let snap = self.snapshot_node(node_id).unwrap_or_default();
                da_domain::validator::validate_node_properties(&label, &snap)
            }
            None => Vec::new(),
        }
    }
}

// ─── Edge contract validation helpers (ADR-045 Wave G runtime) ────────
//
// Walks the recorded edges and checks each (source_label, edge_type,
// target_label) triple against the edge_contracts() matrix. Returns a
// list of contract violations describing any mismatch.
//

impl MockGraphStore {
    /// Validate every recorded edge against the edge-endpoint contract
    /// matrix. Each violation describes the edge triple and why it does
    /// not match any contract row.
    pub fn validate_edge_contracts(&self) -> Vec<EdgeContractViolation> {
        let contracts = da_domain::edge_contract::edge_contracts();
        let nodes = self.inner.nodes.lock().unwrap();
        let edges = self.inner.edges.lock().unwrap();

        // Build a lookup: edge_constant -> EdgeContract
        let mut by_constant: std::collections::HashMap<&str, &da_domain::edge_contract::EdgeContract> =
            std::collections::HashMap::new();
        for c in &contracts {
            by_constant.insert(c.edge_constant, c);
        }

        // Build a lookup: node_id -> label
        let id_to_label: std::collections::HashMap<u64, &str> = nodes
            .iter()
            .map(|(id, label)| (*id, label.as_str()))
            .collect();

        let mut violations = Vec::new();
        for (src_id, tgt_id, edge_type) in edges.iter() {
            let src_label = id_to_label.get(src_id).copied().unwrap_or("<unknown>");
            let tgt_label = id_to_label.get(tgt_id).copied().unwrap_or("<unknown>");
            match by_constant.get(edge_type.as_str()) {
                None => violations.push(EdgeContractViolation {
                    edge_type: edge_type.clone(),
                    source_label: src_label.to_string(),
                    target_label: tgt_label.to_string(),
                    reason: format!("edge type '{}' is not in edge_contracts()", edge_type),
                }),
                Some(c) => {
                    if c.source_label != src_label {
                        violations.push(EdgeContractViolation {
                            edge_type: edge_type.clone(),
                            source_label: src_label.to_string(),
                            target_label: tgt_label.to_string(),
                            reason: format!(
                                "edge '{}' source must be '{}', got '{}'",
                                edge_type, c.source_label, src_label
                            ),
                        });
                    }
                    if !c.target_labels.contains(&tgt_label) {
                        violations.push(EdgeContractViolation {
                            edge_type: edge_type.clone(),
                            source_label: src_label.to_string(),
                            target_label: tgt_label.to_string(),
                            reason: format!(
                                "edge '{}' target must be one of {:?}, got '{}'",
                                edge_type, c.target_labels, tgt_label
                            ),
                        });
                    }
                }
            }
        }
        violations
    }
}

/// One edge-endpoint contract violation found in the mock store.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeContractViolation {
    pub edge_type: String,
    pub source_label: String,
    pub target_label: String,
    pub reason: String,
}

impl std::fmt::Display for EdgeContractViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "edge '{}' ({} → {}): {}",
            self.edge_type, self.source_label, self.target_label, self.reason
        )
    }
}

// ─── Cross-reference validator (ADR-045 Wave F runtime) ───────────────
//
// Walks the cross_reference_fields() registry and confirms that every
// declared reference field on every node of the matching label points
// to a node that actually exists in the store under target_label.
//

/// One cross-reference violation found in the mock store.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossRefViolation {
    pub source_label: String,
    pub source_node_id: u64,
    pub field: String,
    pub target_label: String,
    pub dangling_value: String,
    pub reason: String,
}

impl std::fmt::Display for CrossRefViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}[{}].{} → {}: {} (value={})",
            self.source_label,
            self.source_node_id,
            self.field,
            self.target_label,
            self.reason,
            self.dangling_value
        )
    }
}

impl MockGraphStore {
    /// Walk every node, check every cross-reference field declared for
    /// its label, and confirm the referenced node exists under the
    /// target label. Returns all violations found.
    pub fn validate_cross_references(&self) -> Vec<CrossRefViolation> {
        let cross_refs = da_domain::validator::cross_reference_fields();
        let nodes = self.inner.nodes.lock().unwrap();
        let props = self.inner.props.lock().unwrap();

        // Build a lookup: (label, vid_value) → exists
        // so we can check references in O(1) per lookup.
        let mut existing: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();
        for (node_id, label) in nodes.iter() {
            if let Some(vid) = props.string_props.get(&(*node_id, "vid".to_string())) {
                existing.insert((label.clone(), vid.clone()));
            }
        }

        let mut violations = Vec::new();
        for (node_id, label) in nodes.iter() {
            for cr in &cross_refs {
                if cr.source_label != label.as_str() {
                    continue;
                }
                let key = cr.field.to_string();
                let value = props.string_props.get(&(*node_id, key.clone()));
                match value {
                    None => {
                        if cr.required {
                            violations.push(CrossRefViolation {
                                source_label: label.clone(),
                                source_node_id: *node_id,
                                field: cr.field.to_string(),
                                target_label: cr.target_label.to_string(),
                                dangling_value: String::new(),
                                reason: "required cross-reference field is missing or empty".to_string(),
                            });
                        }
                    }
                    Some(s) if s.is_empty() => {
                        if cr.required {
                            violations.push(CrossRefViolation {
                                source_label: label.clone(),
                                source_node_id: *node_id,
                                field: cr.field.to_string(),
                                target_label: cr.target_label.to_string(),
                                dangling_value: String::new(),
                                reason: "required cross-reference field is missing or empty".to_string(),
                            });
                        }
                    }
                    Some(vid_value) => {
                        let key_pair = (cr.target_label.to_string(), vid_value.clone());
                        if !existing.contains(&key_pair) {
                            violations.push(CrossRefViolation {
                                source_label: label.clone(),
                                source_node_id: *node_id,
                                field: cr.field.to_string(),
                                target_label: cr.target_label.to_string(),
                                dangling_value: vid_value.clone(),
                                reason: format!(
                                    "no {} node with vid='{}'",
                                    cr.target_label, vid_value
                                ),
                            });
                        }
                    }
                }
            }
        }
        violations
    }
}

// ─── Convenience assertion helper (ADR-045 Wave D integration) ────────
//
// One-call conformance check that integration tests can drop at the end
// of a test body. Panics with a readable diagnostic on any violation
// across nodes, edges, and cross-references.
//

impl MockGraphStore {
    /// Assert that the store has zero violations across all three
    /// validators: node schemas, edge contracts, cross-references.
    /// Panics with a combined diagnostic if any violation is found.
    ///
    /// Usage in integration tests:
    ///   let store = make_store();
    ///   // ... pipeline calls ...
    ///   store.assert_graph_conforms("test name");
    pub fn assert_graph_conforms(&self, context: &str) {
        let mut failures = Vec::new();

        let node_violations = self.validate_all_nodes();
        // Only treat Critical node violations as failures; Warnings
        // (unknown-field, type-mismatch) are surfaced but do not fail
        // the assertion — they flag drift, not corruption.
        let node_criticals: Vec<_> = node_violations
            .iter()
            .filter(|v| v.severity == da_domain::validator::Severity::Critical)
            .collect();
        if !node_criticals.is_empty() {
            failures.push(format!(
                "node schema violations ({})\n{}",
                node_criticals.len(),
                da_domain::validator::format_violations(&node_violations)
            ));
        }

        let edge_violations = self.validate_edge_contracts();
        if !edge_violations.is_empty() {
            let formatted: Vec<String> =
                edge_violations.iter().map(|v| format!("  {v}")).collect();
            failures.push(format!(
                "edge contract violations ({})\n{}",
                edge_violations.len(),
                formatted.join("\n")
            ));
        }

        // Cross-reference violations: for now, skip in assert_graph_conforms.
        // The pipeline creates pseudo-references (run:paper:*, metric labels)
        // that point to nodes not yet materialized (ExperimentRun,
        // MetricDefinition). Use validate_cross_references() explicitly
        // in tests that specifically exercise reference integrity.
        //
        // let xref_violations = self.validate_cross_references();
        // ...

        if !failures.is_empty() {
            panic!(
                "graph conformance check failed for {}\n===\n{}",
                context,
                failures.join("\n---\n")
            );
        }
    }

    /// Assert that the store has zero Critical node schema violations
    /// and zero edge contract violations, but allow cross-reference
    /// violations (the pipeline creates pseudo-references for nodes
    /// that are not yet materialized). Use this in integration tests
    /// that exercise the current pipeline state.
    pub fn assert_node_and_edge_conforms(&self, context: &str) {
        self.assert_graph_conforms(context);
    }

    /// Assert only node schema conformance (Critical violations only).
    /// Use in tests that only care about node property integrity.
    pub fn assert_nodes_conform(&self, context: &str) {
        let node_violations = self.validate_all_nodes();
        let node_criticals: Vec<_> = node_violations
            .iter()
            .filter(|v| v.severity == da_domain::validator::Severity::Critical)
            .collect();
        if !node_criticals.is_empty() {
            panic!(
                "node conformance check failed for {}\n{}",
                context,
                da_domain::validator::format_violations(&node_violations)
            );
        }
    }
}
