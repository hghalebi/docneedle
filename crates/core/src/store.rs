use crate::models::{PdfChunk, SearchCandidate, SearchQuery};

pub type SearchHit = SearchCandidate;

#[derive(Debug, Clone)]
pub struct StoreHit {
    pub source: String,
    pub score: f64,
    pub chunk: Option<PdfChunk>,
    pub chunk_id: String,
    pub text: String,
}

impl StoreHit {
    pub fn into_candidate(self, mode: crate::models::SearchMode) -> SearchCandidate {
        SearchCandidate {
            chunk_id: self.chunk_id,
            document_id: String::new(),
            source_path: String::new(),
            score: self.score,
            source: self.source,
            chunk: self.chunk,
            text: Some(self.text),
            mode,
        }
    }
}

pub struct SearchRequest<'a> {
    pub query: &'a SearchQuery,
}
