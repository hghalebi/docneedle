use crate::error::IngestError;
use crate::models::{ChunkKind, DocumentFingerprint, IngestionOptions, PdfChunk};
use regex::Regex;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy)]
pub struct ChunkingConfig {
    pub max_chars: usize,
    pub overlap_chars: usize,
    pub min_chars: usize,
}

impl From<IngestionOptions> for ChunkingConfig {
    fn from(value: IngestionOptions) -> Self {
        Self {
            max_chars: value.chunk_max_chars,
            overlap_chars: value.chunk_overlap_chars,
            min_chars: value.min_chunk_chars,
        }
    }
}

pub fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('\u{a0}', " ")
}

pub fn chunk_by_paragraph(normalized: &str, config: ChunkingConfig) -> Vec<String> {
    let raw_paragraphs = normalized
        .split("\n\n")
        .map(|paragraph| paragraph.trim().replace('\t', " "))
        .filter(|paragraph| !paragraph.trim().is_empty())
        .collect::<Vec<_>>();

    let mut chunks = Vec::new();
    let mut current = String::new();

    for paragraph in raw_paragraphs {
        if current.is_empty() {
            current.push_str(&paragraph);
            continue;
        }

        if current.len() + paragraph.len() + 2 <= config.max_chars {
            current.push_str("\n\n");
            current.push_str(&paragraph);
        } else {
            if current.len() >= config.min_chars {
                chunks.push(current.clone());
            }
            current.clear();
            current.push_str(&paragraph);
        }
    }

    if current.len() >= config.min_chars {
        chunks.push(current);
    }

    if chunks.is_empty() && !normalized.trim().is_empty() {
        chunks.push(normalized.trim().to_string());
    }

    let mut with_overlap = Vec::new();
    for chunk in chunks {
        if chunk.len() <= config.max_chars {
            with_overlap.push(chunk);
            continue;
        }

        let chars: Vec<char> = chunk.chars().collect();
        let mut start = 0;
        while start < chars.len() {
            let end = (start + config.max_chars).min(chars.len());
            let piece: String = chars[start..end].iter().collect();
            with_overlap.push(piece);
            if end == chars.len() {
                break;
            }
            start = start.saturating_add(config.max_chars.saturating_sub(config.overlap_chars));
        }
    }

    with_overlap
}

pub fn build_chunks(
    document: &DocumentFingerprint,
    page: u32,
    section_context: &str,
    clause_id: Option<String>,
    page_text: &str,
    options: &IngestionOptions,
    global_index: u64,
) -> Result<(Vec<PdfChunk>, u64), IngestError> {
    let config = ChunkingConfig::from(options.clone());
    let normalized = normalize_whitespace(page_text);
    let section_heading_re = Regex::new(options.section_heading_regex)?;
    let clause_re = Regex::new(options.clause_regex)?;

    let mut chunks = Vec::new();
    let mut cursor = global_index;

    for raw_chunk in chunk_by_paragraph(&normalized, config) {
        if raw_chunk.trim().len() < config.min_chars {
            continue;
        }

        let first_line = raw_chunk
            .lines()
            .next()
            .map(|line| line.trim().to_string())
            .unwrap_or_default();

        let clause_match = clause_re
            .captures(&first_line)
            .and_then(|capture| capture.get(0).map(|m| m.as_str().to_string()));

        let final_section = if section_heading_re.is_match(&first_line) {
            first_line.clone()
        } else {
            section_context.to_string()
        };

        let chunk_id = make_chunk_id(&document.document_id, page, cursor, &raw_chunk);

        chunks.push(PdfChunk {
            chunk_id,
            document_id: document.document_id.clone(),
            source_path: document.source_path.clone(),
            title: document.document_title.clone(),
            version: document.version.clone(),
            standard: document.standard.clone(),
            section_path: final_section.clone(),
            clause_id: clause_match.or_else(|| clause_id.clone()),
            page_start: page,
            page_end: page,
            chunk_index: cursor,
            text_raw: raw_chunk.clone(),
            text_normalized: normalize_whitespace(&raw_chunk),
            kind: if section_heading_re.is_match(&first_line) {
                ChunkKind::Heading
            } else {
                ChunkKind::Paragraph
            },
            ocr_confidence: None,
            references: Vec::new(),
            units: extract_unit_tokens(&raw_chunk),
        });

        cursor = cursor.saturating_add(1);
    }

    Ok((chunks, cursor))
}

fn make_chunk_id(document_id: &str, page: u32, index: u64, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(document_id.as_bytes());
    hasher.update(page.to_le_bytes());
    hasher.update(index.to_le_bytes());
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn extract_unit_tokens(text: &str) -> Vec<String> {
    const UNITS: [&str; 11] = [
        "mm", "cm", "m", "in", "psi", "bar", "kpa", "pa", "%", "rpm", "hz",
    ];
    let lowered = text.to_lowercase();
    UNITS
        .iter()
        .filter_map(|unit| {
            if lowered.contains(unit) {
                Some((*unit).to_string())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_normalized() {
        let input = "A  \t  lot\nof   spacing";
        let normalized = normalize_whitespace(input);
        assert_eq!(normalized, "A lot of spacing");
    }

    #[test]
    fn chunking_produces_minimum_chunk_size_chunks() {
        let options = IngestionOptions {
            chunk_max_chars: 20,
            chunk_overlap_chars: 4,
            min_chunk_chars: 5,
            section_heading_regex: r"(?m)^Section",
            clause_regex: r"(?m)^Clause",
        };

        let document = DocumentFingerprint {
            document_id: "doc-1".to_string(),
            document_title: "Test".to_string(),
            source_path: "/tmp/test.pdf".to_string(),
            version: None,
            standard: None,
            checksum: "checksum".to_string(),
            ingested_at: chrono::Utc::now(),
        };

        let page_text = "Section 1\n\nSome long paragraph with numbers and terms.";
        let result = build_chunks(&document, 1, "Section 1", None, page_text, &options, 0)
            .unwrap()
            .0;

        assert!(!result.is_empty());
        assert_eq!(result[0].document_id, "doc-1");
        assert!(
            result[0].kind == super::ChunkKind::Heading || result[0].kind == ChunkKind::Paragraph
        );
    }
}
