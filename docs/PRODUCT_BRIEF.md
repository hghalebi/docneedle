# Product Brief

## Vision

Build a high-confidence PDF retrieval tool for industrial documentation where traceability matters as much as recall.

## Problem

Industrial instruction manuals and regulations contain:

- mixed precision language,
- references across sections,
- many short acronyms,
- tables and clauses with strict compliance semantics.

Single-mode search often fails or ranks weakly in this context.

## Solution

A three-layer retrieval architecture:

- keyword layer for exact terminology,
- vector layer for semantic overlap,
- graph layer for citation and linkage.

Weights are fused to balance precision and recall while preserving evidence provenance.

## Business value

- Better decision support for engineers and QA teams.
- Faster locate-and-verify workflows with chunk-level citations.
- Better audit trail from `document_id` and `source_path` to returned snippets.

## Operational model

- Docker bootstrap initializes all required services.
- Idempotent setup creates indexes/schema only when missing.
- Best-effort ingestion avoids pipeline stoppage on single broken PDFs.

## Current risks

- Additional infrastructure increases ops overhead.
- OCR endpoint availability impacts scanned archival document recall.
- Relevance quality depends on chunking and filter configuration.

## Success metrics

- Median query-to-first-result latency in local environments.
- Ingestion completion rate after best-effort skips are reported.
- Percentage of matched evidence with valid source path and document id.
