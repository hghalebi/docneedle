use crate::{
    build_chunks, chunking::normalize_whitespace, extract_page_texts, DocumentFingerprint,
    IngestError, IngestionOptions, PdfChunk,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn discover_pdf_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in WalkDir::new(folder)
        .into_iter()
        .filter_map(|item| item.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let is_pdf = entry
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));

        if is_pdf {
            files.push(entry.path().to_path_buf());
        }
    }

    files.sort_unstable();
    files
}

pub fn digest_file(path: &Path) -> Result<String, IngestError> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn ingest_folder_chunks(
    folder: &Path,
    options: IngestionOptions,
) -> Result<Vec<PdfChunk>, IngestError> {
    let report = ingest_folder_chunks_best_effort(folder, options)?;
    Ok(report.chunks)
}

pub struct SkippedPdf {
    pub path: PathBuf,
    pub reason: String,
}

pub struct IngestionReport {
    pub chunks: Vec<PdfChunk>,
    pub skipped_files: Vec<SkippedPdf>,
}

pub fn ingest_folder_chunks_best_effort(
    folder: &Path,
    options: IngestionOptions,
) -> Result<IngestionReport, IngestError> {
    let files = discover_pdf_files(folder);

    if files.is_empty() {
        return Err(IngestError::InvalidArgument(format!(
            "no pdf files found in {}",
            folder.display()
        )));
    }

    let mut result = Vec::new();
    let mut skipped_files = Vec::new();
    let mut cursor = 0u64;

    for path in files {
        let build_result = (|| {
            let fingerprint = build_document_fingerprint(&path)?;
            let pages = extract_page_texts(&path)?;
            let mut chunks = Vec::new();

            for page in pages {
                let normalized = normalize_whitespace(&page.text);
                let (page_chunks, next_cursor) = build_chunks(
                    &fingerprint,
                    page.number,
                    "unassigned",
                    None,
                    &normalized,
                    &options,
                    cursor,
                )?;

                cursor = next_cursor;
                chunks.extend(page_chunks);
            }

            Ok::<_, IngestError>(chunks)
        })();

        match build_result {
            Ok(file_chunks) => result.extend(file_chunks),
            Err(error) => skipped_files.push(SkippedPdf {
                path,
                reason: error.to_string(),
            }),
        }
    }

    Ok(IngestionReport {
        chunks: result,
        skipped_files,
    })
}

fn build_document_fingerprint(path: &Path) -> Result<DocumentFingerprint, IngestError> {
    let checksum = digest_file(path)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            IngestError::MissingFileName(format!("path missing filename: {}", path.display()))
        })?;

    Ok(DocumentFingerprint {
        document_id: generate_document_id(path),
        document_title: name.to_string(),
        source_path: path.to_string_lossy().to_string(),
        version: None,
        standard: None,
        checksum,
        ingested_at: Utc::now(),
    })
}

fn generate_document_id(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{digest_file, discover_pdf_files, ingest_folder_chunks_best_effort};
    use crate::IngestionOptions;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn discover_pdf_files_is_recursive() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let base = dir.path();
        let nested = base.join("nested");
        fs::create_dir(&nested)?;

        File::create(base.join("a.pdf")).and_then(|mut file| file.write_all(b"%PDF-1.4\n%fake"))?;
        File::create(nested.join("b.pdf"))
            .and_then(|mut file| file.write_all(b"%PDF-1.4\n%fake"))?;

        let files = discover_pdf_files(base);
        assert_eq!(files.len(), 2);
        Ok(())
    }

    #[test]
    fn checksum_is_reproducible() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let file_path = dir.path().join("a.pdf");
        fs::write(&file_path, b"abc")?;

        let first = digest_file(&file_path)?;
        let second = digest_file(&file_path)?;
        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn ingestion_fails_without_pdfs() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let options = IngestionOptions::default();
        let result = ingest_folder_chunks_best_effort(dir.path(), options);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn best_effort_skips_unreadable_pdfs() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let options = IngestionOptions::default();
        fs::write(dir.path().join("unreadable.pdf"), b"%PDF-1.4\n%broken")?;

        let report = ingest_folder_chunks_best_effort(dir.path(), options)?;

        assert_eq!(report.chunks.len(), 0);
        assert_eq!(report.skipped_files.len(), 1);
        assert_eq!(
            report.skipped_files[0]
                .path
                .file_name()
                .and_then(|name| name.to_str()),
            Some("unreadable.pdf")
        );
        Ok(())
    }
}
