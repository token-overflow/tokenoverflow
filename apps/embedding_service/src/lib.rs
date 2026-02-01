#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

// Embedding Service
// OpenAI-compatible embedding endpoint using fastembed-rs for local development.
//
// This service provides real semantic embeddings without requiring external API calls,
// making it suitable for local development and integration testing.

pub mod api;
pub mod embedder;
pub mod logging;
pub mod model;
pub mod types;
