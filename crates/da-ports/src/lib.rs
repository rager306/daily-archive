//! da-ports — trait definitions (ports in hexagonal architecture).
//!
//! ADR-037 §2: Ports layer depends on Domain only.
//! These traits define the boundaries between our application and external systems.

pub mod graph_store;
pub mod evidence_store;
pub mod embedder;
pub mod parser;
pub mod llm_client;

pub use graph_store::GraphStore;
pub use evidence_store::EvidenceStore;
pub use embedder::Embedder;
pub use parser::ParserPort;
pub use llm_client::{LLMClient, ChatMessage, ChatResponse, ChatOptions};
