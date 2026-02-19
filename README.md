# docneedle

## About docneedle

docneedle is a Rust workspace for industrial-document search where traceability is as important as relevance.
It combines:
- exact terminology matching (keyword search),
- semantic similarity (vector search),
- and citation-aware graph expansion (reference relationships).

The project is designed for engineering teams that need reliable answer traces from manuals, standards, and requirement documents,
with resilient ingestion for large or imperfect PDF libraries.

## Documentation

- [Project docs index](./docs/README.md)
- [Architecture decisions](./docs/ARCHITECTURE_DECISIONS.md)
- [Developer guide](./docs/DEVELOPER_GUIDE.md)
- [User guide](./docs/USER_GUIDE.md)
- [Troubleshooting](./docs/TROUBLESHOOTING.md)
- [Learning notes](./docs/LEARNING_NOTES.md)
- [Product brief](./docs/PRODUCT_BRIEF.md)

## Build and release check

```bash
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Current project identity

- Public brand: `docneedle`
- Workspace crate names currently used in code: `pdf-search-core` and `pdf-search-cli`

## CI quality gate

This repository includes a GitHub Actions workflow:

- `.github/workflows/code-quality.yml`

The workflow runs automatically on:

- pushes to `main` / `master`
- all pull requests

It enforces:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo doc --workspace --no-deps --all-features --document-private-items`

Before publishing, update:
- Maintainer contact and repo links
- Version history in `CHANGELOG.md`
- Any local-only scripts or credentials in `.env.example` only

This repository includes a Rust workspace that ingests PDFs and searches them across:

- OpenSearch for keyword retrieval
- Qdrant for vector retrieval
- Neo4j for reference-graph expansion

## One-command bootstrap

Use the bootstrap script to start third-party services (optional) and configure them
if they are not already initialized:

```bash
cp .env.example .env
just bootstrap
```

Or use the Just targets:

```bash
just bootstrap
just bootstrap-watch
```

What the script does:

- optionally starts `opensearch`, `qdrant`, and `neo4j` from `deploy/docker-compose.yml` when `AUTO_START_STACK=true`
- waits for each service to become healthy
- creates OpenSearch index `pdf_chunks` when missing
- creates Qdrant collection `pdf_chunks` with dimension `128` when missing
- creates Neo4j unique constraints and indexes when missing
- if Neo4j auth is custom, set `NEO4J_AUTH` in `.env` (or keep `NEO4J_USER` and `NEO4J_PASSWORD` and let bootstrap compute it).

## Existing stack and custom URLs

If you already run services elsewhere, skip docker startup and point the script to
the existing endpoints:

```bash
AUTO_START_STACK=false \
OPENSEARCH_URL=http://your-opensearch:9200 \
QDRANT_URL=http://your-qdrant:6333 \
NEO4J_URL=http://your-neo4j:7474 \
NEO4J_USER=neo4j \
NEO4J_PASSWORD=secure-pass \
NEO4J_AUTH=neo4j/secure-pass \
just bootstrap
```

The script is intentionally idempotent: rerunning it does not recreate indexes
that already exist.

### Multimodal OCR fallback for unreadable PDFs

If a PDF has no readable text, ingestion can call a multimodal OCR endpoint
automatically instead of skipping it.

Set these optional environment variables before running ingest:

```bash
LLM_OCR_ENDPOINT=http://localhost:8080/ocr
LLM_OCR_API_KEY=your-api-key
```

The endpoint is expected to return JSON in one of these forms:

```json
{ "text": "..." }
```

or

```json
{ "pages": [{ "page": 1, "text": "..." }, { "page": 2, "text": "..." }] }
```

If no endpoint is configured, ingestion falls back to the existing PDF text extraction
and logs a skip for unreadable files.

## Troubleshooting: `curl: (7) Failed to connect to localhost port 9200`

If OpenSearch does not come up:

1. Start the stack manually and keep it running:

```bash
docker compose -f deploy/docker-compose.yml up -d
docker compose -f deploy/docker-compose.yml logs -f opensearch
```

2. Confirm the service is in the compose app and retry bootstrap:

```bash
docker compose -f deploy/docker-compose.yml ps
bash scripts/bootstrap-stack.sh
```

3. On Linux hosts, OpenSearch commonly requires this sysctl setting:

```bash
sudo sysctl -w vm.max_map_count=262144
```

4. If you have low memory or short startup budgets, rerun with a larger timeout:

```bash
SETUP_TIMEOUT_SECONDS=600 bash scripts/bootstrap-stack.sh
```
