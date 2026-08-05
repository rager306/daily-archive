//! da-application — use cases and orchestrators.
//!
//! ADR-037 §2: Application layer depends on Domain + Ports.
//! ADR-037 §4: Data flows (ingest, extraction, agent, ETL scheduler).
//! ADR-040 §11.3: Migration framework.

pub mod ingest;

pub use ingest::IngestUseCase;
pub mod batch_ingest;
pub use batch_ingest::{BatchIngestResult, batch_ingest_pdfs};
pub mod extraction;
pub use extraction::{ExtractionResult, ExtractionUseCase};
pub mod healing;
pub use healing::GraphHealingUseCase;
pub mod enrich;
pub use enrich::{EnrichResult, EnrichUseCase};
pub mod scheduler;
pub use scheduler::GraphScheduler;
pub mod cluster;
pub use cluster::{ClusterResult, ClusterUseCase};
pub mod queries;
pub use queries::{EntityQueries, PaperQueries, RelationQueries};
pub mod schema_init;
pub use schema_init::{CURRENT_SCHEMA_VERSION, SchemaInitializer};
pub mod graph_inventory;
pub use graph_inventory::{GraphInventory, collect_graph_inventory, render_inventory_markdown};
pub mod temporal_stamp;
pub use temporal_stamp::{
    create_edge_with_temporal, edge_type_is_temporal_fact, open_fact_edge_now,
    stamp_edge_temporal_fields,
};
