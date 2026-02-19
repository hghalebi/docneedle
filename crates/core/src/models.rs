use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFingerprint {
    pub document_id: String,
    pub document_title: String,
    pub source_path: String,
    pub version: Option<String>,
    pub standard: Option<String>,
    pub checksum: String,
    pub ingested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ChunkKind {
    Paragraph,
    Heading,
    Table,
    Figure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfChunk {
    pub chunk_id: String,
    pub document_id: String,
    pub source_path: String,
    pub title: String,
    pub version: Option<String>,
    pub standard: Option<String>,
    pub section_path: String,
    pub clause_id: Option<String>,
    pub page_start: u32,
    pub page_end: u32,
    pub chunk_index: u64,
    pub text_raw: String,
    pub text_normalized: String,
    pub kind: ChunkKind,
    pub ocr_confidence: Option<f32>,
    pub references: Vec<String>,
    pub units: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub struct QueryFilters {
    pub standard: Option<String>,
    pub version: Option<String>,
    pub section_path: Option<String>,
    pub clause_id: Option<String>,
    pub path_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SearchQuery {
    pub text: String,
    pub top_k: usize,
    pub mandatory_terms: Vec<String>,
    pub must_not_terms: Vec<String>,
    pub filters: QueryFilters,
    pub explain: bool,
}

impl SearchQuery {
    pub fn all_terms_required(&self) -> Vec<String> {
        if !self.mandatory_terms.is_empty() {
            self.mandatory_terms.clone()
        } else {
            self.text
                .split_whitespace()
                .map(|token| token.to_lowercase())
                .filter(|token| token.len() > 2)
                .collect()
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SearchMode {
    Keyword,
    Vector,
    Graph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCandidate {
    pub chunk_id: String,
    pub document_id: String,
    pub source_path: String,
    pub score: f64,
    pub source: String,
    pub chunk: Option<PdfChunk>,
    pub text: Option<String>,
    pub mode: SearchMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub query: String,
    pub mode_scores: Vec<(String, usize, f64)>,
    pub hits: Vec<SearchCandidate>,
}

#[derive(Debug, Clone)]
pub struct IngestionOptions {
    pub chunk_max_chars: usize,
    pub chunk_overlap_chars: usize,
    pub min_chunk_chars: usize,
    pub section_heading_regex: &'static str,
    pub clause_regex: &'static str,
}

impl Default for IngestionOptions {
    fn default() -> Self {
        Self {
            chunk_max_chars: 1_200,
            chunk_overlap_chars: 120,
            min_chunk_chars: 120,
            section_heading_regex: r"(?m)^\s*\d+(?:\.\d+)*(?:\([a-zA-Z]\))?\s+.+$",
            clause_regex: r"(?m)^\s*\d+(?:\.\d+)*(?:\([a-zA-Z0-9]+\))?\s+[A-Za-z].+$",
        }
    }
}
