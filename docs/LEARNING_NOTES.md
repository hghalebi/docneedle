# Learning Notes

## What you can learn from this codebase

- How to run retrieval pipelines over heterogeneous stores.
- How to design async trait boundaries in Rust for pluggable backends.
- How to model domain metadata for traceable search evidence.
- How to handle imperfect data with best-effort processing.
- How to layer observability and structured CLI output.

## Rust syntax and features used

- `enum` and `match` for mode dispatch.
- `Result<T, E>` and typed errors.
- `impl Trait` and generic structs (`SearchCoordinator<K, V, G>`).
- `async/await`, `tokio::try_join!`, and `#[async_trait]`.
- `serde` derive for serializable domain models.

## Key concepts/patterns

- Dependency Inversion: `docneedle-cli` (package `pdf-search-cli`) depends on abstractions exposed by `docneedle-core` (package `pdf-search-core`).
- Strategy pattern: different store implementations satisfy shared traits.
- Deterministic IDs and reproducibility: document fingerprint and chunking cursors.
- RAII-style resource lifetime through CLI scopes and async clients.
- Error propagation: one error enum per domain (`IngestError`, `SearchError`).

## Suggested study path

1. Start with `crates/core/src/models.rs`.
2. Read `crates/core/src/traits.rs`.
3. Understand rank fusion in `crates/core/src/orchestrator.rs`.
4. Follow `crates/core/src/ingest.rs` for end-to-end chunking flow.
5. Compare store adapters in `crates/core/src/stores/*`.
