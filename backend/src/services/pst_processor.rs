// Added: PST file processor service for TMAIL-115
// PURPOSE: Background processing of uploaded PST files — converts via readpst, then IMAP APPENDs
// EXTERNAL: Requires `readpst` CLI tool (from libpst package) installed on the system
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::AppError;

/// PURPOSE: Holds configuration and state for PST processing operations
/// CONSTRAINTS: readpst must be available in PATH; output directory must be writable
pub struct PstProcessor {
    upload_dir: PathBuf,
}

impl PstProcessor {
    /// PURPOSE: Create a new PstProcessor with the given upload directory
    pub fn new(upload_dir: PathBuf) -> Self {
        Self { upload_dir }
    }

    /// PURPOSE: Get the path to a stored PST file by import ID
    pub fn pst_file_path(&self, import_id: Uuid) -> PathBuf {
        self.upload_dir.join(format!("{}.pst", import_id))
    }

    /// PURPOSE: Get the output directory where readpst will extract .eml files
    pub fn output_dir(&self, import_id: Uuid) -> PathBuf {
        self.upload_dir.join(format!("{}_output", import_id))
    }

    /// PURPOSE: Run readpst to extract emails from a PST file into individual .eml files
    /// CONSTRAINTS: readpst must be installed (apt install pst-utils on Debian/Ubuntu)
    /// NOTE: Uses -e flag for individual .eml output, -o for output directory
    pub async fn extract_emails(
        &self,
        import_id: Uuid,
    ) -> Result<Vec<PathBuf>, AppError> {
        let pst_path = self.pst_file_path(import_id);
        let output_path = self.output_dir(import_id);

        // Added: Verify the PST file exists before attempting extraction
        if !pst_path.exists() {
            return Err(AppError::NotFound(format!(
                "PST file not found at '{}'. It may have been deleted or the upload failed.",
                pst_path.display()
            )));
        }

        // Added: Create output directory for extracted .eml files
        tokio::fs::create_dir_all(&output_path)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to create extraction output directory '{}': {}",
                    output_path.display(),
                    e
                ))
            })?;

        // Added: Run readpst subprocess to convert PST to individual .eml files
        let readpst_output = tokio::process::Command::new("readpst")
            .arg("-e")                      // NOTE: Extract to individual .eml files
            .arg("-o")
            .arg(output_path.to_str().unwrap_or("/tmp"))
            .arg(pst_path.to_str().unwrap_or(""))
            .output()
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!(
                    "Failed to execute readpst command. Is libpst/pst-utils installed? Error: {}",
                    e
                ))
            })?;

        if !readpst_output.status.success() {
            let stderr = String::from_utf8_lossy(&readpst_output.stderr);
            return Err(AppError::Internal(anyhow::anyhow!(
                "readpst failed with exit code {:?}: {}",
                readpst_output.status.code(),
                stderr
            )));
        }

        // Added: Collect all extracted .eml files from the output directory (recursive)
        let eml_files = collect_eml_files(&output_path)?;

        Ok(eml_files)
    }

    /// PURPOSE: Clean up extracted files and the uploaded PST after processing
    pub async fn cleanup(&self, import_id: Uuid) -> Result<(), AppError> {
        let output_path = self.output_dir(import_id);
        let pst_path = self.pst_file_path(import_id);

        // Added: Remove extraction output directory
        if output_path.exists() {
            let _ = tokio::fs::remove_dir_all(&output_path).await;
        }

        // Added: Remove uploaded PST file
        if pst_path.exists() {
            let _ = tokio::fs::remove_file(&pst_path).await;
        }

        Ok(())
    }
}

/// PURPOSE: Recursively collect all .eml files from a directory tree
/// NOTE: readpst may create subdirectories for different PST folders
fn collect_eml_files(dir: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut eml_files = Vec::new();
    let entries = std::fs::read_dir(dir)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to read directory '{}': {}", dir.display(), e)))?;

    for entry in entries {
        let entry = entry
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to read directory entry: {}", e)))?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            // Added: Recurse into subdirectories (readpst creates folder-based subdirs)
            let mut sub_files = collect_eml_files(&entry_path)?;
            eml_files.append(&mut sub_files);
        } else if let Some(ext) = entry_path.extension() {
            if ext == "eml" {
                eml_files.push(entry_path);
            }
        }
    }

    Ok(eml_files)
}

/// PURPOSE: Read the contents of an .eml file as raw bytes for IMAP APPEND
pub async fn read_eml_file(path: &Path) -> Result<Vec<u8>, AppError> {
    tokio::fs::read(path)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!(
            "Failed to read EML file '{}': {}",
            path.display(),
            e
        )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pst_file_path_construction() {
        // Added: Verify PST file path is built correctly from import ID
        let processor = PstProcessor::new(PathBuf::from("/tmp/pst_uploads"));
        let import_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        let pst_path = processor.pst_file_path(import_id);
        assert_eq!(
            pst_path.to_str().unwrap(),
            "/tmp/pst_uploads/550e8400-e29b-41d4-a716-446655440000.pst"
        );
    }

    #[test]
    fn test_output_dir_construction() {
        // Added: Verify output directory path uses _output suffix
        let processor = PstProcessor::new(PathBuf::from("/tmp/pst_uploads"));
        let import_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        let output_path = processor.output_dir(import_id);
        assert_eq!(
            output_path.to_str().unwrap(),
            "/tmp/pst_uploads/550e8400-e29b-41d4-a716-446655440000_output"
        );
    }

    #[test]
    fn test_processor_new() {
        // Added: Verify PstProcessor construction accepts any valid path
        let processor = PstProcessor::new(PathBuf::from("/var/data/pst"));
        let test_id = Uuid::nil();
        assert!(processor.pst_file_path(test_id).starts_with("/var/data/pst"));
    }

    #[tokio::test]
    async fn test_extract_emails_missing_file() {
        // Added: Verify extraction fails gracefully when PST file doesn't exist
        let processor = PstProcessor::new(PathBuf::from("/tmp/pst_test_nonexistent"));
        let fake_id = Uuid::new_v4();

        let result = processor.extract_emails(fake_id).await;
        assert!(result.is_err());
        let error_message = format!("{}", result.unwrap_err());
        assert!(error_message.contains("not found") || error_message.contains("Not found"));
    }

    #[tokio::test]
    async fn test_collect_eml_files_empty_dir() {
        // Added: Verify collect_eml_files returns empty vec for empty directory
        let temp_dir = tempfile::tempdir().unwrap();
        let eml_files = collect_eml_files(temp_dir.path()).unwrap();
        assert!(eml_files.is_empty());
    }

    #[tokio::test]
    async fn test_collect_eml_files_with_eml_files() {
        // Added: Verify collect_eml_files finds .eml files in directory
        let temp_dir = tempfile::tempdir().unwrap();
        let eml_path = temp_dir.path().join("message1.eml");
        let txt_path = temp_dir.path().join("readme.txt");
        tokio::fs::write(&eml_path, b"From: test@example.com").await.unwrap();
        tokio::fs::write(&txt_path, b"Not an email").await.unwrap();

        let eml_files = collect_eml_files(temp_dir.path()).unwrap();
        assert_eq!(eml_files.len(), 1);
        assert!(eml_files[0].ends_with("message1.eml"));
    }

    #[tokio::test]
    async fn test_collect_eml_files_recursive() {
        // Added: Verify collect_eml_files recurses into subdirectories
        let temp_dir = tempfile::tempdir().unwrap();
        let sub_dir = temp_dir.path().join("Inbox");
        tokio::fs::create_dir_all(&sub_dir).await.unwrap();
        tokio::fs::write(sub_dir.join("msg1.eml"), b"Email 1").await.unwrap();
        tokio::fs::write(sub_dir.join("msg2.eml"), b"Email 2").await.unwrap();
        tokio::fs::write(temp_dir.path().join("top.eml"), b"Top email").await.unwrap();

        let eml_files = collect_eml_files(temp_dir.path()).unwrap();
        assert_eq!(eml_files.len(), 3);
    }

    #[tokio::test]
    async fn test_read_eml_file_success() {
        // Added: Verify read_eml_file returns correct bytes
        let temp_dir = tempfile::tempdir().unwrap();
        let eml_path = temp_dir.path().join("test.eml");
        let content = b"From: sender@example.com\r\nSubject: Test\r\n\r\nBody";
        tokio::fs::write(&eml_path, content).await.unwrap();

        let result = read_eml_file(&eml_path).await.unwrap();
        assert_eq!(result, content.to_vec());
    }

    #[tokio::test]
    async fn test_read_eml_file_not_found() {
        // Added: Verify read_eml_file fails for missing file
        let result = read_eml_file(Path::new("/tmp/nonexistent_eml_file_12345.eml")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cleanup_nonexistent_files() {
        // Added: Verify cleanup doesn't error when files already gone
        let processor = PstProcessor::new(PathBuf::from("/tmp/pst_cleanup_test"));
        let fake_id = Uuid::new_v4();
        let result = processor.cleanup(fake_id).await;
        assert!(result.is_ok());
    }
}
