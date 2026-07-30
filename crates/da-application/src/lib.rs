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
