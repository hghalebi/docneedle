# docneedle CLI

CLI entrypoint for the docneedle PDF retrieval stack.
(Cargo package name in this repository is currently `pdf-search-cli`.)

## Usage

- Ingest PDFs:

```bash
cargo run -p pdf-search-cli -- ingest --folder ./pdfs
```

- Search chunks:

```bash
cargo run -p pdf-search-cli -- search --query "pump pressure" --top-k 10
```

- Include full document extraction in the output:

```bash
cargo run -p pdf-search-cli -- search --query "pump pressure" --include-document-text
```
