use crate::{PdfChunk, SearchCandidate, SearchError, SearchMode};
use crate::traits::GraphIndex;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

pub struct Neo4jStore {
    endpoint: String,
    database: String,
    username: String,
    password: String,
    client: Client,
}

impl Neo4jStore {
    pub fn new(
        endpoint: impl Into<String>,
        database: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            database: database.into(),
            username: username.into(),
            password: password.into(),
            client: Client::new(),
        }
    }

    fn tx_url(&self) -> String {
        format!("{}/db/{}/tx/commit", self.endpoint, self.database)
    }
}

#[async_trait]
impl GraphIndex for Neo4jStore {
    async fn sync_graph_relations(&self, chunks: &[PdfChunk]) -> Result<(), SearchError> {
        if chunks.is_empty() {
            return Ok(());
        }

        let rows: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                json!({
                    "doc_id": chunk.document_id,
                    "chunk_id": chunk.chunk_id,
                    "source": chunk.source_path,
                    "section_path": chunk.section_path,
                    "clause_id": chunk.clause_id,
                    "text": chunk.text_raw,
                })
            })
            .collect();

        let cypher = r#"
            UNWIND $rows AS row
            MERGE (doc:Document {document_id: row.doc_id})
            MERGE (c:Chunk {chunk_id: row.chunk_id})
            SET c.source_path = row.source,
                doc.source_path = row.source,
                c.section_path = row.section_path,
                c.clause_id = row.clause_id,
                c.text = row.text
            MERGE (doc)-[:HAS_CHUNK]->(c)
            RETURN count(c) AS chunk_count;
        "#;

        let response = self
            .client
            .post(self.tx_url())
            .basic_auth(&self.username, Some(&self.password))
            .json(&json!({
                "statements": [
                    {
                        "statement": cypher,
                        "parameters": { "rows": rows }
                    }
                ]
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SearchError::BackendResponse {
                backend: "neo4j".to_string(),
                details: response.status().to_string(),
            });
        }

        Ok(())
    }

    async fn related_chunks(&self, chunk_ids: &[String]) -> Result<Vec<SearchCandidate>, SearchError> {
        if chunk_ids.is_empty() {
            return Ok(Vec::new());
        }

        let query = r#"
            MATCH (c:Chunk)-[:REFERENCES]->(ref:Clause)
            WHERE c.chunk_id IN $chunk_ids
            OPTIONAL MATCH (ref)-[:CITED_BY]->(related:Clause)
            MATCH (d:Document)-[:HAS_CHUNK]->(rchunk:Chunk)
            WHERE rchunk.chunk_id = related.clause_id OR rchunk.section_path = related.section
            RETURN DISTINCT c.chunk_id AS from_chunk_id,
                            rchunk.chunk_id AS related_chunk_id,
                            coalesce(rchunk.text, '') AS text,
                            rchunk.section_path AS section,
                            rchunk.source_path AS source_path,
                            d.document_id AS document_id
            LIMIT 20;
        "#;

        let response = self
            .client
            .post(self.tx_url())
            .basic_auth(&self.username, Some(&self.password))
            .json(&json!({
                "statements": [
                    {
                        "statement": query,
                        "parameters": {"chunk_ids": chunk_ids}
                    }
                ]
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SearchError::BackendResponse {
                backend: "neo4j".to_string(),
                details: response.status().to_string(),
            });
        }

        let body: Value = response.json().await?;
        let rows = extract_rows(&body);

        let mut hits = Vec::new();
        for row in rows {
                if let Some(values) = row.as_array() {
                if values.len() >= 6 {
                    let chunk_id = values
                        .get(1)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let text = values.get(2).and_then(Value::as_str).unwrap_or_default().to_string();
                    let source_path = values
                        .get(4)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let document_id = values
                        .get(5)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    hits.push(SearchCandidate {
                        chunk_id,
                        document_id,
                        source_path,
                        score: 0.6,
                        source: "neo4j".to_string(),
                        chunk: None,
                        text: Some(text),
                        mode: SearchMode::Graph,
                    });
                }
            }
        }

        Ok(hits)
    }
}

fn extract_rows(payload: &Value) -> Vec<&Value> {
    let data = payload.pointer("results").and_then(Value::as_array);
    match data {
        Some(results) => results
            .iter()
            .filter_map(|result| result.pointer("data").and_then(Value::as_array))
            .flat_map(|result_rows| {
                result_rows
                    .iter()
                    .filter_map(|row_entry| {
                        row_entry
                            .pointer("row")
                            .or(Some(row_entry))
                            .filter(|candidate| Value::is_array(*candidate))
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
            })
            .collect(),
        None => payload
            .pointer("data")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row_entry| {
                        row_entry
                            .pointer("row")
                            .or(Some(row_entry))
                            .filter(|candidate| Value::is_array(*candidate))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    }
}
