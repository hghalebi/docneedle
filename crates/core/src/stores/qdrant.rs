use crate::{PdfChunk, SearchCandidate, SearchError, SearchMode, SearchQuery};
use crate::traits::{KeywordIndex, VectorIndex};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

pub struct QdrantStore {
    endpoint: String,
    collection: String,
    client: Client,
    vector_size: usize,
}

impl QdrantStore {
    pub fn new(endpoint: impl Into<String>, collection: impl Into<String>, vector_size: usize) -> Self {
        Self {
            endpoint: endpoint.into(),
            collection: collection.into(),
            client: Client::new(),
            vector_size,
        }
    }

    pub fn ensure_collection(&self, vector_size: usize) -> Result<(), SearchError> {
        if self.vector_size != vector_size {
            return Err(SearchError::Request(format!(
                "configured vector size {} does not match requested {}",
                self.vector_size, vector_size
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl KeywordIndex for QdrantStore {
    async fn index_keyword_chunks(&self, _chunks: &[PdfChunk]) -> Result<(), SearchError> {
        Ok(())
    }

    async fn search_keyword(&self, _query: &SearchQuery) -> Result<Vec<SearchCandidate>, SearchError> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl VectorIndex for QdrantStore {
    async fn index_vector_chunks(
        &self,
        chunks: &[PdfChunk],
        embeddings: &[Vec<f32>],
    ) -> Result<(), SearchError> {
        if chunks.len() != embeddings.len() {
            return Err(SearchError::Request(format!(
                "embedding count {} doesn't match chunk count {}",
                embeddings.len(),
                chunks.len()
            )));
        }

        let points = chunks
            .iter()
            .zip(embeddings.iter())
            .map(|(chunk, embedding)| {
                if embedding.len() != self.vector_size {
                    return Err(SearchError::Request(format!(
                        "embedding dimension {} != {}",
                        embedding.len(),
                        self.vector_size
                    )));
                }

                let payload = json!({
                    "document_id": chunk.document_id,
                    "source_path": chunk.source_path,
                    "section_path": chunk.section_path,
                    "clause_id": chunk.clause_id,
                    "page_start": chunk.page_start,
                    "page_end": chunk.page_end,
                    "chunk_index": chunk.chunk_index,
                    "text_raw": chunk.text_raw,
                    "kind": format!("{:?}", chunk.kind),
                    "ocr_confidence": chunk.ocr_confidence,
                    "references": chunk.references,
                    "version": chunk.version,
                    "standard": chunk.standard,
                });

                Ok(json!({
                    "id": chunk.chunk_index,
                    "vector": embedding,
                    "payload": payload,
                }))
            })
            .collect::<Result<Vec<_>, SearchError>>()?;

        if points.is_empty() {
            return Ok(());
        }

        let response = self
            .client
            .put(format!(
                "{}/collections/{}/points?wait=true",
                self.endpoint, self.collection
            ))
            .json(&json!({ "points": points }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SearchError::BackendResponse {
                backend: "qdrant".to_string(),
                details: response.status().to_string(),
            });
        }

        Ok(())
    }

    async fn search_vector(
        &self,
        query_vector: &[f32],
        query: &SearchQuery,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        if query_vector.len() != self.vector_size {
            return Err(SearchError::Request(format!(
                "query vector dim {} is not {}",
                query_vector.len(),
                self.vector_size
            )));
        }

        let response = self
            .client
            .post(format!(
                "{}/collections/{}/points/search",
                self.endpoint, self.collection
            ))
            .json(&json!({
                "vector": query_vector,
                "limit": query.top_k,
                "with_payload": true,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SearchError::BackendResponse {
                backend: "qdrant".to_string(),
                details: response.status().to_string(),
            });
        }

        let parsed: Value = response.json().await?;
        let hits = parsed
            .pointer("/result")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut result = Vec::new();
        for hit in hits {
            let id = hit
                .pointer("/id")
                .and_then(Value::as_u64)
                .map(|id| id.to_string())
                .unwrap_or_default();
            let source_path = hit
                .pointer("/payload/source_path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let document_id = hit
                .pointer("/payload/document_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let score = hit.pointer("/score").and_then(Value::as_f64).unwrap_or(0.0);
            let text = hit
                .pointer("/payload/text_raw")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            result.push(SearchCandidate {
                chunk_id: id,
                document_id,
                source_path,
                score,
                source: "qdrant".to_string(),
                chunk: None,
                text: Some(text),
                mode: SearchMode::Vector,
            });
        }

        Ok(result)
    }
}
