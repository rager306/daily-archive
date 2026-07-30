//! da-ports — trait definitions (ports in hexagonal architecture).
//!
//! ADR-037 §2: Ports layer depends on Domain only.
//! These traits define the boundaries between our application and external systems.

pub mod algorithms;
pub mod embedder;
pub mod evidence_store;
pub mod extractor;
pub mod graph_store;
pub mod llm_client;
pub mod openalex;
pub mod parser;

pub use algorithms::GraphAlgorithms;
pub use embedder::Embedder;
pub use evidence_store::EvidenceStore;
pub use extractor::{ExtractedEntity, Extractor};
pub use graph_store::{DirectGraphStore, GraphStore};
pub use llm_client::{ChatMessage, ChatOptions, ChatResponse, LLMClient};
pub use openalex::{OpenAlexAuthor, OpenAlexClient, OpenAlexConcept, OpenAlexTopic, OpenAlexWork};
pub use parser::ParserPort;
