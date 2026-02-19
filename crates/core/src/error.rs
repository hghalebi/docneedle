use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("pdf parse error: {0}")]
    PdfParse(String),

    #[error("regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("path has no file name: {0}")]
    MissingFileName(String),

    #[error("invalid chunking config: {0}")]
    InvalidChunkConfig(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("multimodal OCR failed: {0}")]
    OcrFailed(String),
}

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("invalid response from {backend}: {details}")]
    BackendResponse { backend: String, details: String },

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("url parse error: {0}")]
    Url(#[from] url::ParseError),

    #[error("serialize error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("search request failed: {0}")]
    Request(String),

    #[error("store not available yet: {0}")]
    NotReady(String),
}

pub type Result<T, E = IngestError> = std::result::Result<T, E>;
