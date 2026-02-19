# Developer Guide

## Repository layout

- `crates/core`: domain logic, models, store adapters, orchestration.
- `crates/app`: CLI and runtime wiring (`docneedle-cli`, crate name: `pdf-search-cli`).
- `scripts`: bootstrap automation.
- `deploy`: docker-compose services used by local stack.
- `examples`: sample PDFs.

## Module map

- `extractor.rs`: PDF extraction and OCR fallback.
- `chunking.rs`: text chunking and normalization.
- `ingest.rs`: folder traversal and chunk orchestration.
- `models.rs`: shared domain models (`SearchCandidate`, `SearchQuery`, `PdfChunk`, metadata).
- `traits.rs`: async trait contracts for keyword/vector/graph stores.
- `orchestrator.rs`: coordinator and rank fusion logic.
- `stores/`: adapters for OpenSearch, Qdrant, Neo4j.
- `error.rs`: typed errors via `thiserror`.

## Design patterns used

- Strategy pattern by trait abstraction:
  each store implements the same search/index contract.
- Dependency inversion via generic coordinator:
  `SearchCoordinator<K, V, G>` receives store implementations.
- Best-effort ingestion:
  recover from per-file failures and continue.
- Layered search:
  parallel keyword/vector retrieval plus graph expansion before fusion.

## Data contracts

- `SearchQuery` includes
  - text,
  - `top_k`,
  - mandatory and blocked terms,
  - filters (`standard`, `version`, `section_path`, `clause_id`, `path_prefix`),
  - explain flag.
- `SearchCandidate` includes `chunk_id`, `document_id`, `source_path`, score, source, and optional chunk/text payload.
- `SearchResult` returns query echo, per-mode score metadata, and final ranked hits.

## Local verification

- `cargo test`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo doc --workspace --no-deps --all-features --document-private-items`

## CI quality gate

- GitHub Actions workflow: `.github/workflows/code-quality.yml`
- Triggers on `push` and `pull_request` for `main` / `master` (plus manual dispatch).
- Enforces formatting, compile, clippy, tests, and documentation build.

## Extending retrieval modes

To add another backend:

1. Add trait implementation type in `crates/core/src/traits.rs` contract.
2. Add store adapter under `crates/core/src/stores/`.
3. Extend coordinator fusion list in `orchestrator.rs`.
4. Update result docs and troubleshooting references.

## Security and reliability notes

- Keep `LLM_OCR_ENDPOINT` and API keys in environment.
- The extractor currently blocks on endpoint I/O only in OCR path.
- Watch chunk IDs and `document_id` consistency if chunking rules change.
