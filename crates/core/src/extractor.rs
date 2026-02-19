use crate::error::IngestError;
use base64::{engine::general_purpose::STANDARD, Engine};
use lopdf::Document;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PageText {
    pub number: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
struct LlmOcrRequest {
    pdf_base64: String,
    source_path: String,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmOcrResponse {
    pages: Option<Vec<LlmOcrPage>>,
    text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LlmOcrPage {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OcrEndpointConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
}

pub trait PdfExtractor {
    fn extract_pages(&self, path: &Path) -> Result<Vec<PageText>, IngestError>;
}

#[derive(Default)]
pub struct LopdfExtractor;

impl PdfExtractor for LopdfExtractor {
    fn extract_pages(&self, path: &Path) -> Result<Vec<PageText>, IngestError> {
        let document = Document::load(path).map_err(|error| IngestError::PdfParse(error.to_string()))?;

        let mut pages = Vec::new();
        for (page_no, _page_id) in document.get_pages() {
            let text = document
                .extract_text(&[page_no])
                .map_err(|error| IngestError::PdfParse(error.to_string()))?;

            if !text.trim().is_empty() {
                pages.push(PageText {
                    number: page_no,
                    text,
                });
            }
        }

        if pages.is_empty() {
            return Err(IngestError::PdfParse(format!(
                "pdf had no readable page text: {}",
                path.display()
            )));
        }

        Ok(pages)
    }
}

pub fn extract_page_texts(path: &Path) -> Result<Vec<PageText>, IngestError> {
    let extracted = LopdfExtractor::default().extract_pages(path);

    match extracted {
        Ok(pages) => Ok(pages),
        Err(IngestError::PdfParse(parse_error)) => {
            match extract_with_llm_ocr(path) {
                Ok(Some(pages)) => Ok(pages),
                Ok(None) => Err(IngestError::PdfParse(parse_error)),
                Err(ocr_error) => Err(IngestError::PdfParse(format!(
                    "{parse_error}; multimodal OCR fallback failed: {ocr_error}"
                ))),
            }
        }
        Err(error) => Err(error),
    }
}

fn parse_llm_ocr_config() -> Option<OcrEndpointConfig> {
    let endpoint = std::env::var("LLM_OCR_ENDPOINT").ok()?;
    let endpoint = endpoint.trim().to_string();
    if endpoint.is_empty() {
        return None;
    }

    let api_key = std::env::var("LLM_OCR_API_KEY")
        .ok()
        .and_then(|value| {
            let key = value.trim().to_string();
            if key.is_empty() {
                None
            } else {
                Some(key)
            }
        });

    Some(OcrEndpointConfig { endpoint, api_key })
}

fn extract_with_llm_ocr(path: &Path) -> Result<Option<Vec<PageText>>, IngestError> {
    tokio::task::block_in_place(|| extract_with_llm_ocr_blocking(path))
}

fn extract_with_llm_ocr_blocking(path: &Path) -> Result<Option<Vec<PageText>>, IngestError> {
    let cfg = match parse_llm_ocr_config() {
        Some(cfg) => cfg,
        None => return Ok(None),
    };

    let pdf = std::fs::read(path).map_err(IngestError::Io)?;
    let payload = LlmOcrRequest {
        pdf_base64: STANDARD.encode(pdf),
        source_path: path.to_string_lossy().to_string(),
    };

    let mut request = Client::new()
        .post(&cfg.endpoint)
        .header("content-type", "application/json")
        .json(&payload);

    if let Some(api_key) = cfg.api_key {
        request = request.bearer_auth(api_key);
    }

    let response = request.send()?;

    if !response.status().is_success() {
        return Err(IngestError::OcrFailed(format!(
            "multimodal OCR request to {} returned {}",
            cfg.endpoint,
            response.status()
        )));
    }

    let payload: LlmOcrResponse = response.json()?;
    let pages = payload_to_pages(&payload, path)?;

    if pages.is_empty() {
        return Err(IngestError::OcrFailed(format!(
            "multimodal OCR response has no readable text: {}",
            path.display()
        )));
    }

    Ok(Some(pages))
}

fn payload_to_pages(payload: &LlmOcrResponse, path: &Path) -> Result<Vec<PageText>, IngestError> {
    if let Some(listed) = &payload.pages {
        let listed = listed
            .iter()
            .filter_map(|page| {
                let text = page.text.as_ref().map(|value| value.trim().to_string());
                text.and_then(|normalized| {
                    if normalized.is_empty() {
                        None
                    } else {
                        let page_number = page.page.unwrap_or(1);
                        Some(PageText {
                            number: page_number,
                            text: normalized,
                        })
                    }
                })
            })
            .collect::<Vec<_>>();

        if !listed.is_empty() {
            return Ok(listed);
        }
    }

    if let Some(raw_text) = &payload.text {
        let pages = raw_text
            .split('\u{000c}')
            .enumerate()
            .filter_map(|(index, chunk)| {
                let normalized = chunk.trim().to_string();
                if normalized.is_empty() {
                    None
                } else {
                    Some(PageText {
                        number: (index + 1) as u32,
                        text: normalized,
                    })
                }
            })
            .collect::<Vec<_>>();

        if !pages.is_empty() {
            return Ok(pages);
        }
    }

    Err(IngestError::OcrFailed(format!(
        "multimodal OCR response was empty for {}",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::{payload_to_pages, LlmOcrPage, LlmOcrResponse};
    use std::path::Path;

    #[test]
    fn ocr_payload_with_pages_converts_only_nonempty_text() {
        let response = LlmOcrResponse {
            pages: Some(vec![
                LlmOcrPage {
                    page: Some(2),
                    text: Some("  ".to_string()),
                },
                LlmOcrPage {
                    page: Some(3),
                    text: Some("Page 3".to_string()),
                },
            ]),
            text: None,
        };

        let pages = payload_to_pages(&response, Path::new("x.pdf"))
            .expect("multimodal response should be parsed");

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].number, 3);
        assert_eq!(pages[0].text, "Page 3");
    }

    #[test]
    fn ocr_payload_fallback_text_split_by_form_feed() {
        let response = LlmOcrResponse {
            pages: None,
            text: Some("First\u{000C}Second\n".to_string()),
        };

        let pages = payload_to_pages(&response, Path::new("x.pdf"))
            .expect("multimodal response should be parsed");

        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].number, 1);
        assert_eq!(pages[0].text, "First");
        assert_eq!(pages[1].number, 2);
        assert_eq!(pages[1].text, "Second");
    }
}
