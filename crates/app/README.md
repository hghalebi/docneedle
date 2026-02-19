# docneedle CLI

CLI entrypoint for the docneedle PDF retrieval stack. Package name in this repository: `pdf-search-cli`.

## Usage

- Ingest PDFs:

```bash
DOCNEEDLE_CLI=pdf-search-cli
cargo run -p "$DOCNEEDLE_CLI" -- ingest --folder ./pdfs
```

- Search chunks:

```bash
DOCNEEDLE_CLI=pdf-search-cli
cargo run -p "$DOCNEEDLE_CLI" -- search --query "pump pressure" --top-k 10
```

- Include full document extraction in the output:

```bash
DOCNEEDLE_CLI=pdf-search-cli
cargo run -p "$DOCNEEDLE_CLI" -- search --query "pump pressure" --include-document-text
```
