# docneedle Core

`docneedle-core` is a Rust library (crate name `pdf-search-core`) that provides PDF ingestion and retrieval
building blocks for a multi-layer industrial search stack.

## Core responsibilities

- Extract and chunk PDF text.
- Build deterministic chunk metadata (`document_id`, `source_path`, normalized text).
- Define search candidate/result/domain models.
- Provide async interfaces for keyword, vector, and graph stores.
- Implement rank fusion in `SearchCoordinator`.

## Highlights

- best-effort recursive ingestion,
- multimodal OCR fallback integration,
- multi-store candidate fusion,
- explicit typed errors.

## Versioning

The crate follows semver through its `Cargo.toml` version.
