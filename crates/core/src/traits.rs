use crate::{
    SearchError, SearchQuery, SearchCandidate,
};
use async_trait::async_trait;

#[async_trait]
pub trait KeywordIndex {
    async fn index_keyword_chunks(&self, chunks: &[crate::PdfChunk]) -> Result<(), SearchError>;

    async fn search_keyword(
        &self,
        query: &SearchQuery,
    ) -> Result<Vec<SearchCandidate>, SearchError>;
}

#[async_trait]
pub trait VectorIndex {
    async fn index_vector_chunks(
        &self,
        chunks: &[crate::PdfChunk],
        embeddings: &[Vec<f32>],
    ) -> Result<(), SearchError>;

    async fn search_vector(
        &self,
        query_vector: &[f32],
        query: &SearchQuery,
    ) -> Result<Vec<SearchCandidate>, SearchError>;
}

#[async_trait]
pub trait GraphIndex {
    async fn sync_graph_relations(
        &self,
        chunks: &[crate::PdfChunk],
    ) -> Result<(), SearchError>;

    async fn related_chunks(&self, chunk_ids: &[String]) -> Result<Vec<SearchCandidate>, SearchError>;
}
