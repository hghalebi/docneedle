# User Guide

## What this tool does

This engine indexes PDFs and allows search over extracted content using three retrieval modes:

- keyword,
- semantic,
- graph-based expansion.

The CLI supports two commands:

- `ingest`: parse a folder and push chunks to all stores.
- `search`: query all modes and return ranked evidence.

## Prerequisites

- Docker
- Rust 1.76+
- Justfile support

## Quick start

1. Create local env file:

```bash
cp .env.example .env
```

2. Boot services and initialize schema/indexes:

```bash
just bootstrap
```

3. Ingest recursive folder:

```bash
cargo run -p pdf-search-cli -- ingest --folder ./pdfs
```

4. Search:

```bash
cargo run -p pdf-search-cli -- search --query "pump pressure" --top-k 5
```

5. Show full document text for returned hits:

```bash
cargo run -p pdf-search-cli -- search --query "pump" --include-document-text --document-text-max-pages 2
```

## Command options

### Ingest

- `--folder <PATH>`: folder path containing PDFs (recursive).

### Search

- `--query <TEXT>`: query text.
- `--top-k <N>`: result limit.
- `--explain`: prints mode weights and internal scoring window.
- `--include-document-text`: prints source document page text for unique documents.
- `--document-text-max-pages`: maximum pages output per document.

## Interpreting results

Each hit currently prints:

- backend source,
- score,
- chunk id,
- `document_id`,
- source path (where available),
- chunk text.

## Operational guidance

- Keep `AUTO_START_STACK=true` while iterating locally.
- Use `AUTO_START_STACK=false` when backing services already run externally.
- Keep endpoint credentials in `.env` only; do not commit secrets.
