//! da-application — use cases and orchestrators.
//!
//! ADR-037 §2: Application layer depends on Domain + Ports.
//! ADR-037 §4: Data flows (ingest, extraction, agent, ETL scheduler).
//! ADR-040 §11.3: Migration framework.

pub mod ingest;

pub use ingest::IngestUseCase;
