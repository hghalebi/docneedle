# Troubleshooting Guide

## 1) OpenSearch 9200 connection errors

Error:
- `curl: (7) Failed to connect to localhost port 9200`

Check:
- `docker compose -f deploy/docker-compose.yml ps`
- `docker compose -f deploy/docker-compose.yml logs -f opensearch`

Fixes:
- Start with `just bootstrap` after `AUTO_START_STACK=true`.
- Increase timeout: `SETUP_TIMEOUT_SECONDS=600 just bootstrap`.
- Ensure memory/sysctl requirements are satisfied on Linux.

## 2) No hits returned

Check:
- Query is not empty.
- `top_k` is set too high and returns mostly low-confidence noise.
- Filters are overly narrow (`--search` currently supports query filters in model defaults but CLI currently passes defaults).

Fixes:
- Retry with simpler query text.
- Validate ingestion produced chunks.
- Run `cargo run -p pdf-search-cli -- ingest --folder ./pdfs` and inspect logs for skipped files. (Project alias: docneedle CLI.)

## 3) OCR fallback not triggered

Expected path:
- PDFs with no readable text attempt `LLM_OCR_ENDPOINT`.

Check:
- `.env` includes `LLM_OCR_ENDPOINT`.
- endpoint returns `{ "text": "..." }` or `{ "pages": [...] }` JSON.

Fixes:
- Inspect HTTP response and authentication header behavior.
- Verify endpoint network reachability from host.

## 4) Bootstrap setup fails on service schema/index

- OpenSearch permissions/index state problems:
  - Re-run with fresh logs: `just bootstrap-watch`
- Qdrant collection dimension mismatch:
  - Match `EMBEDDING_DIMENSIONS` to `CharacterNgramEmbedder::dimensions()`.
- Neo4j auth mismatch:
  - Keep `NEO4J_AUTH=neo4j/your-password` in `.env`.

## 5) Missing source citations in output

- Output should include `document_id` and `source` when metadata is present.

Fixes:
- Reindex after updates to chunking or extractor behavior:
  - `cargo run -p pdf-search-cli -- ingest --folder ./pdfs` (docneedle CLI)
- Ensure source files are still readable at ingestion time.

## Escalation

If issue persists after checks, capture:
- `just bootstrap-watch` logs,
- CLI command,
- exact query,
- sample file names,
- first 20 lines of the relevant error message,
and share these in the issue report.
