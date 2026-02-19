pub mod chunking;
pub mod embeddings;
pub mod error;
pub mod extractor;
pub mod ingest;
pub mod models;
pub mod orchestrator;
pub mod store;
pub mod stores;
pub mod traits;

pub use chunking::{build_chunks, chunk_by_paragraph, normalize_whitespace, ChunkingConfig};
pub use embeddings::{CharacterNgramEmbedder, Embedder, DEFAULT_EMBEDDING_DIMENSIONS};
pub use error::{IngestError, SearchError};
pub use extractor::{extract_page_texts, PageText, PdfExtractor};
pub use ingest::{
    discover_pdf_files, ingest_folder_chunks, ingest_folder_chunks_best_effort, IngestionReport,
    SkippedPdf,
};
pub use models::{
    ChunkKind, DocumentFingerprint, IngestionOptions, PdfChunk, QueryFilters, SearchCandidate,
    SearchMode, SearchQuery, SearchResult,
};
pub use orchestrator::SearchCoordinator;
pub use stores::{Neo4jStore, OpenSearchStore, QdrantStore};
pub use traits::{GraphIndex, KeywordIndex, VectorIndex};
