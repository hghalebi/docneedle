use crate::{traits::KeywordIndex, SearchCandidate, SearchError, SearchMode, SearchQuery};
use crate::models::PdfChunk;
use crate::traits::VectorIndex;
use async_trait::async_trait;
use reqwest::Client;
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct OpenSearchStore {
    client: Arc<Client>,
    endpoint: String,
    index_name: String,
}

impl OpenSearchStore {
    pub fn new(endpoint: impl Into<String>, index_name: impl Into<String>) -> Self {
        Self {
            client: Arc::new(Client::new()),
            endpoint: endpoint.into(),
            index_name: index_name.into(),
        }
    }

    pub async fn ensure_index(&self) -> Result<(), SearchError> {
        let response = self
            .client
            .head(format!("{}/{}", self.endpoint, self.index_name))
            .send()
            .await?;

        if response.status() == StatusCode::OK {
            return Ok(());
        }

        if !response.status().is_client_error() {
            return Err(SearchError::BackendResponse {
                backend: "opensearch".to_string(),
                details: response.status().to_string(),
            });
        }

        let response = self
            .client
            .put(format!("{}/{}", self.endpoint, self.index_name))
            .json(&json!({
                "settings": {
                    "number_of_shards": 1,
                    "number_of_replicas": 0,
                    "analysis": {
                        "analyzer": {
                            "standard_english": {
                                "type": "standard"
                            }
                        }
                    }
                },
                "mappings": {
                    "properties": {
                        "text_raw": {"type": "text", "analyzer": "standard_english"},
                        "text_normalized": {"type": "text", "analyzer": "standard_english"},
                        "section_path": {"type": "keyword"},
                        "document_id": {"type": "keyword"},
                        "source_path": {"type": "keyword"},
                        "clause_id": {"type": "keyword"},
                        "standard": {"type": "keyword"},
                        "version": {"type": "keyword"},
                        "page_start": {"type": "integer"},
                        "page_end": {"type": "integer"},
                        "chunk_index": {"type": "long"}
                    }
                }
            }))
            .send()
            .await?;

        if response.status().is_server_error() || response.status().is_client_error() {
            return Err(SearchError::Request(format!(
                "open-search index setup failed with {}",
                response.status()
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl KeywordIndex for OpenSearchStore {
    async fn index_keyword_chunks(&self, chunks: &[PdfChunk]) -> Result<(), SearchError> {
        let mut operations = Vec::new();

        for chunk in chunks {
            operations.push(json!({
                "index": {
                    "_index": self.index_name,
                    "_id": chunk.chunk_id,
                }
            }));
            operations.push(json!({
                "document_id": chunk.document_id,
                "source_path": chunk.source_path,
                "section_path": chunk.section_path,
                "clause_id": chunk.clause_id,
                "page_start": chunk.page_start,
                "page_end": chunk.page_end,
                "chunk_index": chunk.chunk_index,
                "text_raw": chunk.text_raw,
                "text_normalized": chunk.text_normalized,
                "kind": format!("{:?}", chunk.kind),
                "ocr_confidence": chunk.ocr_confidence,
                "references": chunk.references,
                "units": chunk.units,
                "version": chunk.version,
                "standard": chunk.standard,
            }));
        }

        if operations.is_empty() {
            return Ok(());
        }

        let payload: String = operations
            .into_iter()
            .map(|value| serde_json::to_string(&value))
            .collect::<Result<Vec<_>, serde_json::Error>>()?
            .join("\n")
            + "\n";

        let response = self
            .client
            .post(format!("{}/_bulk", self.endpoint))
            .header("Content-Type", "application/x-ndjson")
            .body(payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SearchError::BackendResponse {
                backend: "opensearch".to_string(),
                details: response.status().to_string(),
            });
        }
        Ok(())
    }

    async fn search_keyword(&self, query: &SearchQuery) -> Result<Vec<SearchCandidate>, SearchError> {
        let body = json!({
            "size": query.top_k,
            "query": {
                "bool": {
                    "must": [
                        {
                            "multi_match": {
                                "query": query.text,
                                "fields": ["text_raw", "text_normalized", "section_path"]
                            }
                        }
                    ],
                    "filter": build_filters(&query.filters)
                }
            },
            "highlight": {
                "fields": {
                    "text_raw": {}
                }
            }
        });

        let response = self
            .client
            .post(format!("{}/{}/_search", self.endpoint, self.index_name))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SearchError::BackendResponse {
                backend: "opensearch".to_string(),
                details: response.status().to_string(),
            });
        }

        let response_json: Value = response.json().await?;
        let hits = response_json
            .pointer("/hits/hits")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut result = Vec::new();

        for raw in hits {
            let source = raw.pointer("_source").cloned().unwrap_or_else(|| Value::Null);
            let chunk_id = raw
                .pointer("/_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let document_id = source
                .pointer("document_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let source_path = source
                .pointer("source_path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            let score = raw.pointer("/_score").and_then(Value::as_f64).unwrap_or(0.0);
            let text = source
                .pointer("text_raw")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            result.push(SearchCandidate {
                chunk_id,
                document_id,
                source_path,
                score,
                source: "opensearch".to_string(),
                chunk: None,
                text: Some(text),
                mode: SearchMode::Keyword,
            });
        }

        Ok(result)
    }
}

#[async_trait]
impl VectorIndex for OpenSearchStore {
    async fn index_vector_chunks(
        &self,
        _chunks: &[PdfChunk],
        _embeddings: &[Vec<f32>],
    ) -> Result<(), SearchError> {
        Ok(())
    }

    async fn search_vector(
        &self,
        _query_vector: &[f32],
        _query: &SearchQuery,
    ) -> Result<Vec<SearchCandidate>, SearchError> {
        Ok(Vec::new())
    }
}

fn build_filters(filters: &crate::models::QueryFilters) -> Vec<Value> {
    let mut predicates = Vec::new();

    if let Some(standard) = &filters.standard {
        predicates.push(json!({"term": {"standard": standard}}));
    }
    if let Some(version) = &filters.version {
        predicates.push(json!({"term": {"version": version}}));
    }
    if let Some(section) = &filters.section_path {
        predicates.push(json!({"term": {"section_path": section}}));
    }
    if let Some(clause) = &filters.clause_id {
        predicates.push(json!({"term": {"clause_id": clause}}));
    }

    predicates
}
