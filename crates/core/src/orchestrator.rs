use crate::traits::{GraphIndex, KeywordIndex, VectorIndex};
use crate::embeddings::{CharacterNgramEmbedder, Embedder};
use crate::{SearchCandidate, SearchError, SearchMode, SearchQuery, SearchResult};
use std::collections::HashMap;

pub struct SearchCoordinator<K, V, G>
where
    K: KeywordIndex,
    V: VectorIndex,
    G: GraphIndex,
{
    keyword: K,
    vector: V,
    graph: G,
    embedder: CharacterNgramEmbedder,
}

impl<K, V, G> SearchCoordinator<K, V, G>
where
    K: KeywordIndex + Send + Sync,
    V: VectorIndex + Send + Sync,
    G: GraphIndex + Send + Sync,
{
    pub fn new(keyword: K, vector: V, graph: G) -> Self {
        Self {
            keyword,
            vector,
            graph,
            embedder: CharacterNgramEmbedder::default(),
        }
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResult, SearchError> {
        if query.text.trim().is_empty() {
            return Err(SearchError::Request("query is empty".to_string()));
        }

        let required_terms = query.all_terms_required();
        let query_vector = self.embedder.embed(&query.text);

        let (keyword_hits, vector_hits) = tokio::try_join!(
            self.keyword.search_keyword(query),
            self.vector.search_vector(&query_vector, query)
        )?;

        let mut scored = HashMap::<String, ScoredHit>::new();
        apply_rrf(&mut scored, &keyword_hits, 0.55);
        apply_rrf(&mut scored, &vector_hits, 0.35);

        let candidate_ids = scored.keys().cloned().collect::<Vec<_>>();
        let graph_hits = self.graph.related_chunks(&candidate_ids).await.unwrap_or_default();
        if !graph_hits.is_empty() {
            apply_rrf(&mut scored, &graph_hits, 0.10);
        }

        let mut final_hits: Vec<ScoredHit> = scored
            .into_values()
            .filter(|hit| term_check(&hit.chunk_text, &required_terms))
            .filter(|hit| !contains_any_term(&hit.chunk_text, &query.must_not_terms))
            .collect();

        final_hits.sort_by(|left, right| right.total_score.total_cmp(&left.total_score));

        let mode_scores = vec![
            ("keyword".to_string(), 0.55),
            ("vector".to_string(), 0.35),
            ("graph".to_string(), if graph_hits.is_empty() { 0.0 } else { 0.10 }),
        ];

        let mode_scores = mode_scores
            .into_iter()
            .map(|(mode, weight)| {
                let top_k = if mode == "graph" { 20 } else { query.top_k };
                (mode, top_k, weight)
            })
            .collect();

        Ok(SearchResult {
            query: query.text.clone(),
            mode_scores,
            hits: final_hits
                .into_iter()
                .take(query.top_k)
                .map(|item| SearchCandidate {
                    chunk_id: item.chunk_id,
                    document_id: item.document_id,
                    source_path: item.source_path,
                    score: item.total_score,
                    source: item.source,
                    chunk: item.chunk,
                    text: Some(item.chunk_text),
                    mode: dominant_mode(&item.modes),
                })
                .collect(),
        })
    }
}

#[derive(Debug)]
struct ScoredHit {
    chunk_id: String,
    document_id: String,
    source_path: String,
    chunk_text: String,
    total_score: f64,
    source: String,
    chunk: Option<crate::models::PdfChunk>,
    modes: Vec<SearchMode>,
}

fn apply_rrf(target: &mut HashMap<String, ScoredHit>, hits: &[SearchCandidate], weight: f64) {
    const K: f64 = 60.0;
    for (position, hit) in hits.iter().enumerate() {
        let rank_component = 1.0 / (K + (position as f64 + 1.0));
        let text = hit.text.clone().unwrap_or_default();
        let mode = mode_from_source(&hit.source);

        let entry = target.entry(hit.chunk_id.clone()).or_insert(ScoredHit {
            chunk_id: hit.chunk_id.clone(),
            document_id: hit.document_id.clone(),
            source_path: hit.source_path.clone(),
            chunk_text: String::new(),
            total_score: 0.0,
            source: hit.source.clone(),
            chunk: hit.chunk.clone(),
            modes: Vec::new(),
        });

        if entry.chunk_text.is_empty() {
            entry.chunk_text = text;
        }

        if !entry.source.contains(&hit.source) {
            if entry.source.is_empty() {
                entry.source = hit.source.clone();
            } else {
                entry.source = format!("{},{}", entry.source, hit.source);
            }
        }
        if entry.document_id.is_empty() {
            entry.document_id = hit.document_id.clone();
        }
        if entry.source_path.is_empty() {
            entry.source_path = hit.source_path.clone();
        }

        entry.total_score += (weight * rank_component) + (hit.score * 0.01);
        if let Some(found_mode) = mode {
            if !entry.modes.contains(&found_mode) {
                entry.modes.push(found_mode);
            }
        }
    }
}

fn mode_from_source(source: &str) -> Option<SearchMode> {
    match source {
        "opensearch" => Some(SearchMode::Keyword),
        "qdrant" => Some(SearchMode::Vector),
        "neo4j" => Some(SearchMode::Graph),
        _ => None,
    }
}

fn dominant_mode(modes: &[SearchMode]) -> SearchMode {
    if modes.contains(&SearchMode::Graph) {
        SearchMode::Graph
    } else if modes.contains(&SearchMode::Vector) {
        SearchMode::Vector
    } else if modes.contains(&SearchMode::Keyword) {
        SearchMode::Keyword
    } else {
        SearchMode::Keyword
    }
}

fn term_check(text: &str, required_terms: &[String]) -> bool {
    let lowered = text.to_lowercase();
    required_terms
        .iter()
        .all(|term| lowered.contains(&term.to_lowercase()))
}

fn contains_any_term(text: &str, blocked: &[String]) -> bool {
    let lowered = text.to_lowercase();
    blocked
        .iter()
        .any(|term| lowered.contains(&term.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{GraphIndex, KeywordIndex, VectorIndex};
    use async_trait::async_trait;

    #[derive(Default)]
    struct FakeKeywordIndex {
        hits: Vec<SearchCandidate>,
    }

    #[derive(Default)]
    struct FakeVectorIndex {
        hits: Vec<SearchCandidate>,
    }

    #[derive(Default)]
    struct FakeGraphIndex {
        hits: Vec<SearchCandidate>,
    }

    #[async_trait]
    impl KeywordIndex for FakeKeywordIndex {
        async fn index_keyword_chunks(&self, _chunks: &[crate::PdfChunk]) -> Result<(), SearchError> {
            Ok(())
        }

        async fn search_keyword(&self, _query: &SearchQuery) -> Result<Vec<SearchCandidate>, SearchError> {
            Ok(self.hits.clone())
        }
    }

    #[async_trait]
    impl VectorIndex for FakeVectorIndex {
        async fn index_vector_chunks(
            &self,
            _chunks: &[crate::PdfChunk],
            _embeddings: &[Vec<f32>],
        ) -> Result<(), SearchError> {
            Ok(())
        }

        async fn search_vector(
            &self,
            _query_vector: &[f32],
            _query: &SearchQuery,
        ) -> Result<Vec<SearchCandidate>, SearchError> {
            Ok(self.hits.clone())
        }
    }

    #[async_trait]
    impl GraphIndex for FakeGraphIndex {
        async fn sync_graph_relations(&self, _chunks: &[crate::PdfChunk]) -> Result<(), SearchError> {
            Ok(())
        }

        async fn related_chunks(&self, _chunk_ids: &[String]) -> Result<Vec<SearchCandidate>, SearchError> {
            Ok(self.hits.clone())
        }
    }

    #[tokio::test]
    async fn coordinator_uses_rrf_fusion_across_modes() {
        let keyword_store = FakeKeywordIndex {
            hits: vec![SearchCandidate {
                chunk_id: "chunk-1".to_string(),
                document_id: "doc-1".to_string(),
                source_path: "/tmp/doc.pdf".to_string(),
                score: 0.9,
                source: "opensearch".to_string(),
                chunk: None,
                text: Some("hydraulic pump failure pressure".to_string()),
                mode: SearchMode::Keyword,
            }],
        };

        let vector_store = FakeVectorIndex {
            hits: vec![SearchCandidate {
                chunk_id: "chunk-1".to_string(),
                document_id: "doc-1".to_string(),
                source_path: "/tmp/doc.pdf".to_string(),
                score: 0.8,
                source: "qdrant".to_string(),
                chunk: None,
                text: Some("hydraulic pump failure pressure".to_string()),
                mode: SearchMode::Vector,
            }],
        };

        let graph_store = FakeGraphIndex {
            hits: vec![SearchCandidate {
                chunk_id: "chunk-2".to_string(),
                document_id: "doc-2".to_string(),
                source_path: "/tmp/other.pdf".to_string(),
                score: 0.5,
                source: "neo4j".to_string(),
                chunk: None,
                text: Some("other chunk".to_string()),
                mode: SearchMode::Graph,
            }],
        };

        let coordinator = SearchCoordinator::new(keyword_store, vector_store, graph_store);
        let query = SearchQuery {
            text: "hydraulic pump".to_string(),
            top_k: 5,
            mandatory_terms: vec!["hydraulic".to_string()],
            must_not_terms: Vec::new(),
            filters: Default::default(),
            explain: false,
        };

        let result = coordinator.search(&query).await.expect("search should succeed");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].chunk_id, "chunk-1");
        assert_eq!(result.hits[0].mode, SearchMode::Vector);
    }
}
