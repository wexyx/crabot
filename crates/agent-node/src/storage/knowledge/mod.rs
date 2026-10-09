//! The durable store for chat records.
//!
//! JSONL is gone: this LadybugDB graph is where every chat record is written and
//! read. Each turn becomes a node carrying the full event row (`raw`), the
//! searchable text, and its `(chat, seq)` identity; summaries are separate nodes
//! with the sequence range they cover and the agent they belong to, linked to the
//! turns they compress via `COVERED_BY`, so sequential reads skip compressed
//! material per agent.

mod commit_status;
mod document_store;
#[cfg(test)]
mod document_tests;
pub(crate) mod documents;
pub(crate) use document_store::DocumentStore;
mod graph;
mod ingest;
mod memory;
mod recall;

pub(crate) use graph::for_project;
pub(crate) use ingest::{persist, write_summary};
pub(crate) use recall::{Hit, latest_seq, recall, recall_range, render};

mod snapshot;
pub(crate) use snapshot::{ContextSnapshot, context_snapshot};
