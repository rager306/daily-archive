//! Shared test support — domain-aware MockGraphStore for integration tests.
//!
//! Wraps `kg_storage::MockGraphStore` (the canonical storage mock) and
//! adds domain-specific validation: schema validation, edge contract
//! checking, and cross-reference verification.
//!
//! Storage behavior (create_node, set_property, find_node, etc.) is
//! delegated to `kg_storage::MockGraphStore` — the single source of
//! truth for the DirectGraphStore + GraphStore contract (ADR-051 §6).
//! This file adds ONLY the domain-specific validation layer.

#![allow(dead_code)]

use std::collections::HashSet;

use da_ports::graph_store::{
    DirectGraphStore, GraphResult, GraphStore, GraphStoreError, QueryResult, VectorMetric,
    VectorSearchResult,
};

/// Domain-aware mock GraphStore — wraps kg_storage::MockGraphStore
/// with schema validation, edge contract checking, and cross-reference
/// verification.
#[derive(Debug, Clone)]
pub struct MockGraphStore {
    inner: kg_storage::MockGraphStore,
}

impl MockGraphStore {
    pub fn new() -> Self {
        Self {
            inner: kg_storage::MockGraphStore::new(),
        }
    }

    pub fn node_count_total(&self) -> usize {
        self.inner.node_count_total()
    }
    pub fn edge_count_total(&self) -> usize {
        self.inner.edge_count_total()
    }
    pub fn snapshot_call_count(&self) -> usize {
        self.inner.snapshot_call_count()
    }
    pub fn import_call_count(&self) -> usize {
        self.inner.import_call_count()
    }
}

impl Default for MockGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

// ─── GraphStore trait: delegate to inner kg_storage::MockGraphStore ──

#[async_trait::async_trait]
impl GraphStore for MockGraphStore {
    async fn query(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        self.inner.query(graph, cypher).await
    }
    async fn query_readonly(&self, graph: &str, cypher: &str) -> GraphResult<QueryResult> {
        self.inner.query_readonly(graph, cypher).await
    }
    async fn create_vector_index(
        &self,
        label: &str,
        property: &str,
        dimensions: usize,
        metric: VectorMetric,
    ) -> GraphResult<()> {
        self.inner
            .create_vector_index(label, property, dimensions, metric)
            .await
    }
    async fn create_property_index(&self, label: &str, property: &str) -> GraphResult<()> {
        self.inner.create_property_index(label, property).await
    }
    async fn vector_search(
        &self,
        label: &str,
        property: &str,
        query_vector: &[f32],
        k: usize,
    ) -> GraphResult<Vec<VectorSearchResult>> {
        self.inner
            .vector_search(label, property, query_vector, k)
            .await
    }
    async fn export_snapshot(&self) -> GraphResult<Vec<u8>> {
        self.inner.export_snapshot().await
    }
    async fn import_snapshot(&self, data: &[u8]) -> GraphResult<()> {
        self.inner.import_snapshot(data).await
    }
    async fn health(&self) -> GraphResult<bool> {
        self.inner.health().await
    }
}

// ─── DirectGraphStore trait: delegate to inner kg_storage::MockGraphStore ──

#[async_trait::async_trait]
impl DirectGraphStore for MockGraphStore {
    async fn create_node(&self, label: &str) -> Result<u64, GraphStoreError> {
        self.inner.create_node(label).await
    }
    async fn create_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Result<u64, GraphStoreError> {
        self.inner.create_edge(source, target, edge_type).await
    }
    async fn add_vector(
        &self,
        label: &str,
        property: &str,
        node_id: u64,
        vector: Vec<f32>,
    ) -> Result<(), GraphStoreError> {
        self.inner
            .add_vector(label, property, node_id, vector)
            .await
    }
    async fn vector_search_direct(
        &self,
        label: &str,
        property: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(u64, f32)>, GraphStoreError> {
        self.inner
            .vector_search_direct(label, property, query, k)
            .await
    }
    async fn set_node_property_string(
        &self,
        node_id: u64,
        key: &str,
        value: String,
    ) -> Result<(), GraphStoreError> {
        self.inner
            .set_node_property_string(node_id, key, value)
            .await
    }
    async fn set_node_property_int(
        &self,
        node_id: u64,
        key: &str,
        value: i64,
    ) -> Result<(), GraphStoreError> {
        self.inner.set_node_property_int(node_id, key, value).await
    }
    async fn set_node_property_bool(
        &self,
        node_id: u64,
        key: &str,
        value: bool,
    ) -> Result<(), GraphStoreError> {
        self.inner.set_node_property_bool(node_id, key, value).await
    }
    async fn set_node_property_float(
        &self,
        node_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError> {
        self.inner
            .set_node_property_float(node_id, key, value)
            .await
    }
    async fn get_node_property_string(&self, node_id: u64, key: &str) -> Option<String> {
        self.inner.get_node_property_string(node_id, key).await
    }
    async fn get_node_property_int(&self, node_id: u64, key: &str) -> Option<i64> {
        self.inner.get_node_property_int(node_id, key).await
    }
    async fn find_node_by_string_property(
        &self,
        label: &str,
        key: &str,
        value: &str,
    ) -> Option<u64> {
        self.inner
            .find_node_by_string_property(label, key, value)
            .await
    }
    async fn get_incoming_edges(&self, node_id: u64) -> Vec<(u64, String)> {
        self.inner.get_incoming_edges(node_id).await
    }
    async fn get_nodes_by_label(&self, label: &str) -> Vec<u64> {
        self.inner.get_nodes_by_label(label).await
    }
    async fn get_edges_between(
        &self,
        source: u64,
        target: u64,
        edge_type: &str,
    ) -> Vec<(u64, String)> {
        self.inner
            .get_edges_between(source, target, edge_type)
            .await
    }
    async fn get_edge_property_string(&self, edge_id: u64, key: &str) -> Option<String> {
        self.inner.get_edge_property_string(edge_id, key).await
    }
    async fn set_edge_property_string(
        &self,
        edge_id: u64,
        key: &str,
        value: &str,
    ) -> Result<(), GraphStoreError> {
        self.inner
            .set_edge_property_string(edge_id, key, value)
            .await
    }
    async fn set_edge_property_float(
        &self,
        edge_id: u64,
        key: &str,
        value: f64,
    ) -> Result<(), GraphStoreError> {
        self.inner
            .set_edge_property_float(edge_id, key, value)
            .await
    }
    async fn node_count(&self) -> usize {
        self.inner.node_count().await
    }
    async fn edge_count(&self) -> usize {
        self.inner.edge_count().await
    }
}

// ─── Domain-specific validation layer ────────────────────────────────

impl MockGraphStore {
    /// Snapshot a node's properties for schema validation.
    pub fn snapshot_node(&self, node_id: u64) -> Option<da_domain::validator::PropertySnapshot> {
        use serde_json::json;
        let mut snap = da_domain::validator::PropertySnapshot::new();
        let mut found = false;

        for ((id, key), val) in self.inner.string_props_snapshot() {
            if id == node_id {
                snap.insert(key.clone(), json!(val));
                found = true;
            }
        }
        for ((id, key), val) in self.inner.int_props_snapshot() {
            if id == node_id {
                snap.insert(key.clone(), json!(val));
                found = true;
            }
        }
        for ((id, key), val) in self.inner.float_props_snapshot() {
            if id == node_id {
                snap.insert(key.clone(), json!(val));
                found = true;
            }
        }
        for ((id, key), val) in self.inner.bool_props_snapshot() {
            if id == node_id {
                snap.insert(key.clone(), json!(val));
                found = true;
            }
        }
        if found { Some(snap) } else { None }
    }

    /// Validate ALL nodes against the ontology schema.
    pub fn validate_all_nodes(&self) -> Vec<da_domain::validator::SchemaViolation> {
        let mut all = Vec::new();
        for (node_id, label) in self.inner.nodes_snapshot() {
            if let Some(snap) = self.snapshot_node(node_id) {
                let mut v = da_domain::validator::validate_node_properties(&label, &snap);
                all.append(&mut v);
            }
        }
        all
    }

    /// Validate a single node against the ontology schema.
    pub fn validate_node(&self, node_id: u64) -> Vec<da_domain::validator::SchemaViolation> {
        let label = self
            .inner
            .nodes_snapshot()
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, l)| l.clone());
        match label {
            Some(l) => {
                let snap = self.snapshot_node(node_id);
                snap.map(|s| da_domain::validator::validate_node_properties(&l, &s))
                    .unwrap_or_default()
            }
            None => Vec::new(),
        }
    }
}

// ─── Edge contract validation ────────────────────────────────────────

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

impl MockGraphStore {
    /// Validate all edges against the edge contract matrix.
    pub fn validate_edge_contracts(&self) -> Vec<EdgeContractViolation> {
        let contracts = da_domain::edge_contract::edge_contracts();
        let nodes = self.inner.nodes_snapshot();
        let id_to_label: std::collections::HashMap<u64, String> = nodes.iter().cloned().collect();

        // Group contracts by edge_type for lookup
        let by_constant: std::collections::HashMap<&str, &da_domain::edge_contract::EdgeContract> =
            contracts.iter().map(|c| (c.edge_constant, c)).collect();

        let mut violations = Vec::new();
        for (_edge_id, src_id, tgt_id, edge_type) in self.inner.edges_snapshot() {
            let src_label = id_to_label
                .get(&src_id)
                .cloned()
                .unwrap_or_else(|| "<unknown>".to_string());
            let tgt_label = id_to_label
                .get(&tgt_id)
                .cloned()
                .unwrap_or_else(|| "<unknown>".to_string());
            match by_constant.get(edge_type.as_str()) {
                None => violations.push(EdgeContractViolation {
                    edge_type: edge_type.clone(),
                    source_label: src_label.clone(),
                    target_label: tgt_label.clone(),
                    reason: format!("edge type '{}' is not in edge_contracts()", edge_type),
                }),
                Some(c) => {
                    if c.source_label != src_label.as_str() {
                        violations.push(EdgeContractViolation {
                            edge_type: edge_type.clone(),
                            source_label: src_label.clone(),
                            target_label: tgt_label.clone(),
                            reason: format!(
                                "edge '{}' source must be '{}', got '{}'",
                                edge_type, c.source_label, src_label
                            ),
                        });
                    }
                    if !c.target_labels.contains(&tgt_label.as_str()) {
                        violations.push(EdgeContractViolation {
                            edge_type: edge_type.clone(),
                            source_label: src_label.clone(),
                            target_label: tgt_label.clone(),
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

// ─── Cross-reference validation ──────────────────────────────────────

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
    /// target label.
    pub fn validate_cross_references(&self) -> Vec<CrossRefViolation> {
        let cross_refs = da_domain::validator::cross_reference_fields();
        let nodes = self.inner.nodes_snapshot();
        let string_props = self.inner.string_props_snapshot();

        // Build a lookup: (label, vid_value) → exists
        let mut existing: HashSet<(String, String)> = HashSet::new();
        for (node_id, label) in &nodes {
            for ((id, key), val) in &string_props {
                if id == node_id && key == "vid" {
                    existing.insert((label.clone(), val.clone()));
                }
            }
        }

        let mut violations = Vec::new();
        for (node_id, label) in &nodes {
            for cr in &cross_refs {
                if cr.source_label != label.as_str() {
                    continue;
                }
                let key = cr.field.to_string();
                let value: Option<&String> = string_props
                    .iter()
                    .find(|((id, k), _)| id == node_id && k == &key)
                    .map(|(_, v)| v);
                match value {
                    None => {
                        if cr.required {
                            violations.push(CrossRefViolation {
                                source_label: label.clone(),
                                source_node_id: *node_id,
                                field: cr.field.to_string(),
                                target_label: cr.target_label.to_string(),
                                dangling_value: String::new(),
                                reason: "required cross-reference field is missing or empty"
                                    .to_string(),
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
                                reason: "required cross-reference field is missing or empty"
                                    .to_string(),
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

// ─── Convenience assertion helpers ───────────────────────────────────

impl MockGraphStore {
    /// Assert that the store has zero Critical node schema violations
    /// and zero edge contract violations. Cross-reference violations
    /// are NOT checked here (the pipeline creates pseudo-references
    /// for nodes not yet materialized). Use validate_cross_references()
    /// explicitly in tests that exercise reference integrity.
    pub fn assert_graph_conforms(&self, context: &str) {
        let mut failures = Vec::new();

        let node_violations = self.validate_all_nodes();
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
            let formatted: Vec<String> = edge_violations.iter().map(|v| format!("  {v}")).collect();
            failures.push(format!(
                "edge contract violations ({})\n{}",
                edge_violations.len(),
                formatted.join("\n")
            ));
        }

        if !failures.is_empty() {
            panic!(
                "graph conformance check failed for {}\n===\n{}",
                context,
                failures.join("\n---\n")
            );
        }
    }

    /// Alias for assert_graph_conforms — node + edge conformance only.
    pub fn assert_node_and_edge_conforms(&self, context: &str) {
        self.assert_graph_conforms(context);
    }

    /// Assert only node schema conformance (Critical violations only).
    pub fn assert_nodes_conform(&self, context: &str) {
        let node_violations = self.validate_all_nodes();
        let node_criticals: Vec<_> = node_violations
            .iter()
            .filter(|v| v.severity == da_domain::validator::Severity::Critical)
            .collect();
        if !node_criticals.is_empty() {
            panic!(
                "node conformance check failed for {}\n===\n{}",
                context,
                da_domain::validator::format_violations(&node_violations)
            );
        }
    }
}
