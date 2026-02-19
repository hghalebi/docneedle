# Contributing to docneedle

Thank you for your interest in improving docneedle. This document explains how to
contribute safely and efficiently.

## Getting started

- Ensure Rust 1.76+ is installed.
- Copy environment defaults:

```bash
cp .env.example .env
```

- Run the workspace bootstrap for local services:

```bash
just bootstrap
```

## Build and test

- `cargo test` (root workspace)
- Optional local checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`

## Coding expectations

- Keep modules small and focused.
- Prefer concrete types and clear names.
- Add tests for changes to behavior, including parsing and ingestion logic.
- Reuse existing CLI flags and avoid breaking backward compatibility unless required.

## Code style

- Prefer straightforward control flow and explicit error handling.
- Follow the established style in this repository (`cargo fmt`).
- Keep dependency direction clean (`app` depends on `core`; avoid cross-dependencies).

## Pull request checklist

- Describe what changed and why.
- Include any risk, migration, or configuration impacts.
- Add/update tests where behavior is changed.
- Update docs when user-facing behavior changes.
