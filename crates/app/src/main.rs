use clap::{Parser, Subcommand};
use chrono::Utc;
use pdf_search_core::{
    ingest_folder_chunks_best_effort, CharacterNgramEmbedder, IngestionOptions, Neo4jStore,
    OpenSearchStore, QdrantStore, SearchCoordinator, SearchQuery, SearchError, VectorIndex,
};
use pdf_search_core::{
    Embedder, GraphIndex, KeywordIndex,
};
use pdf_search_core::extract_page_texts;
use std::collections::HashSet;
use std::path::Path;
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter, prelude::*};

#[derive(Parser)]
#[command(name = "pdf-search-engine", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// OpenSearch base URL
    #[arg(long, default_value = "http://localhost:9200")]
    opensearch_url: String,

    /// OpenSearch index name
    #[arg(long, default_value = "pdf_chunks")]
    opensearch_index: String,

    /// Qdrant base URL
    #[arg(long, default_value = "http://localhost:6333")]
    qdrant_url: String,

    /// Qdrant collection
    #[arg(long, default_value = "pdf_chunks")]
    qdrant_collection: String,

    /// Neo4j HTTP transaction URL
    #[arg(long, default_value = "http://localhost:7474")]
    neo4j_url: String,

    /// Neo4j database name
    #[arg(long, default_value = "neo4j")]
    neo4j_db: String,

    /// Neo4j username
    #[arg(long, default_value = "neo4j")]
    neo4j_user: String,

    /// Neo4j password
    #[arg(long, default_value = "password")]
    neo4j_password: String,
}

#[derive(Subcommand)]
enum Command {
    /// Ingest PDF folder and index chunks into all stores.
    Ingest {
        /// Folder that contains PDFs recursively.
        #[arg(long)]
        folder: String,
    },
    /// Search all layers and return fused evidence with citations.
    Search {
        /// Search query
        #[arg(long)]
        query: String,
        /// Number of candidates to return.
        #[arg(long, default_value = "10")]
        top_k: usize,
        /// Enable explain mode.
        #[arg(long, default_value_t = false)]
        explain: bool,
        /// Print the full extracted text for each matched document.
        #[arg(long, default_value_t = false)]
        include_document_text: bool,
        /// Maximum number of pages to print when document text is requested.
        #[arg(long, default_value = "2")]
        document_text_max_pages: usize,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app_version = env!("CARGO_PKG_VERSION");

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer())
        .init();

    let cli = Cli::parse();

    let query_embedder = CharacterNgramEmbedder::default();
    let keyword = OpenSearchStore::new(&cli.opensearch_url, &cli.opensearch_index);
    let vector = QdrantStore::new(
        &cli.qdrant_url,
        &cli.qdrant_collection,
        query_embedder.dimensions(),
    );
    let graph = Neo4jStore::new(
        &cli.neo4j_url,
        &cli.neo4j_db,
        &cli.neo4j_user,
        &cli.neo4j_password,
    );

    let coordinator = SearchCoordinator::new(keyword, vector, graph);
    info!(
        version = app_version,
        started_at = %Utc::now().to_rfc3339(),
        "pdf-search-engine boot"
    );

    match cli.command {
        Command::Ingest { folder } => {
            let path = std::path::Path::new(&folder);
            let report = ingest_folder_chunks_best_effort(path, IngestionOptions::default())
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let chunks = report.chunks;

            if !report.skipped_files.is_empty() {
                warn!(
                    "skipped_files={} for folder={}",
                    report.skipped_files.len(),
                    folder
                );
                for skipped in report.skipped_files {
                    warn!(path = %skipped.path.display(), reason = %skipped.reason, "skipped pdf");
                }
            }

            if chunks.is_empty() {
                println!("0 chunks ingested (all files were skipped)");
            }

            info!(folder=%folder, chunk_count=%chunks.len(), "ingesting chunks");

            let embeddings: Vec<_> = chunks
                .iter()
                .map(|chunk| query_embedder.embed(&chunk.text_normalized))
                .collect();

            let keyword_store = OpenSearchStore::new(&cli.opensearch_url, &cli.opensearch_index);
            let vector_store = QdrantStore::new(
                &cli.qdrant_url,
                &cli.qdrant_collection,
                query_embedder.dimensions(),
            );
            let graph_store = Neo4jStore::new(
                &cli.neo4j_url,
                &cli.neo4j_db,
                &cli.neo4j_user,
                &cli.neo4j_password,
            );

            keyword_store
                .ensure_index()
                .await
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            vector_store
                .ensure_collection(query_embedder.dimensions())
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;

            keyword_store
                .index_keyword_chunks(&chunks)
                .await
                .map_err(|error: SearchError| anyhow::anyhow!(error.to_string()))?;
            vector_store
                .index_vector_chunks(&chunks, &embeddings)
                .await
                .map_err(|error: SearchError| anyhow::anyhow!(error.to_string()))?;
            graph_store
                .sync_graph_relations(&chunks)
                .await
                .map_err(|error: SearchError| anyhow::anyhow!(error.to_string()))?;

            println!(
                "{} chunks ingested at {}",
                chunks.len(),
                Utc::now().to_rfc3339()
            );
        }
        Command::Search {
            query,
            top_k,
            explain,
            include_document_text,
            document_text_max_pages,
        } => {
            let search_query = SearchQuery {
                text: query,
                top_k,
                mandatory_terms: Vec::new(),
                must_not_terms: Vec::new(),
                filters: Default::default(),
                explain,
            };

            let result = coordinator
                .search(&search_query)
                .await
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;

            println!("query: {}", result.query);
            let mut emitted_documents: HashSet<String> = HashSet::new();
            let mut document_order: Vec<String> = Vec::new();

            for hit in result.hits {
                println!(
                    "[{}] score={:.4} chunk={} document_id={}",
                    hit.source, hit.score, hit.chunk_id, hit.document_id
                );
                if !hit.source_path.is_empty() {
                    println!("  source={}", hit.source_path);
                }
                if let Some(text) = &hit.text {
                    println!("  chunk_text:\n{text}");
                }
                if include_document_text && !hit.source_path.is_empty() {
                    if emitted_documents.insert(hit.source_path.clone()) {
                        document_order.push(hit.source_path);
                    }
                }
            }

            if include_document_text {
                for path in document_order {
                    println!("document_text: path={path}");
                    match extract_page_texts(Path::new(&path)) {
                        Ok(pages) => {
                            for (index, page) in pages.iter().enumerate() {
                                if index >= document_text_max_pages {
                                    break;
                                }
                                if !page.text.trim().is_empty() {
                                    println!("[page {}]\n{}", page.number, page.text);
                                }
                            }

                            if pages.len() > document_text_max_pages {
                                println!("... output truncated to first {document_text_max_pages} page(s)");
                            }
                        }
                        Err(error) => {
                            println!("  unable_to_read_document: {error}");
                        }
                    }
                }
            }

            if explain {
                for (mode, k, score) in result.mode_scores {
                    println!("explain: mode={mode} top_k={k} weight={score:.2}");
                }
            }
        }
    }

    Ok(())
}
