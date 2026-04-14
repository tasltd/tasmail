// Added: Attachment storage and ClamAV scanning service for TMAIL-59
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

/// PURPOSE: Handles file storage on disk and ClamAV virus scanning via Unix socket
/// CONSTRAINTS: storage_dir must exist and be writable; ClamAV socket is optional
/// EXTERNAL: Filesystem for storage, ClamAV daemon via Unix socket for scanning
#[derive(Debug, Clone)]
pub struct AttachmentService {
    storage_dir: PathBuf,
    clamav_socket: Option<String>,
}

impl AttachmentService {
    pub fn new(storage_dir: PathBuf, clamav_socket: Option<String>) -> Self {
        Self {
            storage_dir,
            clamav_socket,
        }
    }

    /// PURPOSE: Store file to disk under {storage_dir}/{mailbox_id}/{uuid}_{filename}
    /// Returns (storage_path, sha256_checksum)
    /// CONSTRAINTS: Creates mailbox subdirectory if it doesn't exist
    pub async fn store_file(
        &self,
        mailbox_id: Uuid,
        data: &[u8],
        filename: &str,
    ) -> anyhow::Result<(String, String)> {
        // Added: Compute SHA-256 checksum for deduplication and integrity verification
        let mut hasher = Sha256::new();
        hasher.update(data);
        let checksum = format!("{:x}", hasher.finalize());

        // Added: Organize files by mailbox_id subdirectory
        let mailbox_dir = self.storage_dir.join(mailbox_id.to_string());
        tokio::fs::create_dir_all(&mailbox_dir).await.map_err(|err| {
            anyhow::anyhow!(
                "Failed to create attachment directory '{}': {}. Check filesystem permissions.",
                mailbox_dir.display(),
                err
            )
        })?;

        // Added: Prefix with UUID to prevent filename collisions
        let safe_filename = sanitize_filename(filename);
        let stored_name = format!("{}_{}", Uuid::new_v4(), safe_filename);
        let file_path = mailbox_dir.join(&stored_name);

        tokio::fs::write(&file_path, data).await.map_err(|err| {
            anyhow::anyhow!(
                "Failed to write attachment file '{}': {}. Disk may be full or path inaccessible.",
                file_path.display(),
                err
            )
        })?;

        let storage_path = file_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Storage path contains invalid UTF-8"))?
            .to_string();

        Ok((storage_path, checksum))
    }

    /// PURPOSE: Scan a file using ClamAV daemon via Unix socket
    /// Returns (scan_status, optional_result_text)
    /// NOTE: If ClamAV is not configured or unavailable, returns "clean" with a note
    pub async fn scan_file(&self, path: &str) -> anyhow::Result<(String, Option<String>)> {
        let socket_path = match &self.clamav_socket {
            Some(s) => s.clone(),
            None => {
                tracing::debug!("ClamAV not configured, skipping scan for '{}'", path);
                return Ok(("clean".to_string(), Some("ClamAV not configured".to_string())));
            }
        };

        // Added: Connect to ClamAV daemon via Unix socket and send SCAN command
        match self.scan_via_socket(&socket_path, path).await {
            Ok(response) => {
                // NOTE: ClamAV SCAN response format: "/path/to/file: OK" or "/path/to/file: VirusName FOUND"
                if response.contains("OK") {
                    Ok(("clean".to_string(), None))
                } else if response.contains("FOUND") {
                    // Added: Extract virus name from ClamAV response
                    let virus_name = response
                        .split(':')
                        .nth(1)
                        .map(|s| s.trim().replace(" FOUND", ""))
                        .unwrap_or_else(|| "Unknown threat".to_string());
                    tracing::warn!(
                        "ClamAV detected threat in '{}': {}",
                        path,
                        virus_name
                    );
                    Ok(("infected".to_string(), Some(virus_name)))
                } else if response.contains("ERROR") {
                    tracing::error!("ClamAV scan error for '{}': {}", path, response);
                    Ok(("error".to_string(), Some(response)))
                } else {
                    // Added: Unexpected response format — treat as error to be safe
                    tracing::warn!("Unexpected ClamAV response for '{}': {}", path, response);
                    Ok(("error".to_string(), Some(format!("Unexpected response: {}", response))))
                }
            }
            Err(err) => {
                tracing::error!(
                    "ClamAV connection failed for '{}': {}. Socket: '{}'",
                    path,
                    err,
                    socket_path
                );
                Ok(("error".to_string(), Some(format!("ClamAV unavailable: {}", err))))
            }
        }
    }

    /// PURPOSE: Send SCAN command to ClamAV daemon and read response
    async fn scan_via_socket(&self, socket_path: &str, file_path: &str) -> anyhow::Result<String> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut stream = tokio::net::UnixStream::connect(socket_path).await?;

        // Added: ClamAV protocol — send "SCAN <path>\n" and read response
        let command = format!("SCAN {}\n", file_path);
        stream.write_all(command.as_bytes()).await?;
        stream.shutdown().await?;

        let mut response = String::new();
        stream.read_to_string(&mut response).await?;

        Ok(response.trim().to_string())
    }

    /// PURPOSE: Read file contents from storage path
    pub async fn read_file(&self, storage_path: &str) -> anyhow::Result<Vec<u8>> {
        tokio::fs::read(storage_path).await.map_err(|err| {
            anyhow::anyhow!(
                "Failed to read attachment file '{}': {}. File may have been deleted or moved.",
                storage_path,
                err
            )
        })
    }

    /// PURPOSE: Delete file from storage
    pub async fn delete_file(&self, storage_path: &str) -> anyhow::Result<()> {
        // NOTE: Ignore "not found" errors — file may already be cleaned up
        match tokio::fs::remove_file(storage_path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("Attachment file already deleted: '{}'", storage_path);
                Ok(())
            }
            Err(err) => Err(anyhow::anyhow!(
                "Failed to delete attachment file '{}': {}",
                storage_path,
                err
            )),
        }
    }
}

/// PURPOSE: Sanitize filename to prevent path traversal and invalid characters
/// CONSTRAINTS: Replaces directory separators and control characters with underscores
fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        // Added: Trim leading dots to prevent hidden files
        .trim_start_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_safe_name() {
        assert_eq!(sanitize_filename("report.pdf"), "report.pdf");
    }

    #[test]
    fn test_sanitize_filename_path_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "_.._etc_passwd");
    }

    #[test]
    fn test_sanitize_filename_hidden_file() {
        assert_eq!(sanitize_filename(".hidden"), "hidden");
    }

    #[test]
    fn test_sanitize_filename_special_chars() {
        assert_eq!(sanitize_filename("file:name?.txt"), "file_name_.txt");
    }

    #[test]
    fn test_sanitize_filename_windows_separators() {
        assert_eq!(sanitize_filename("path\\to\\file.txt"), "path_to_file.txt");
    }

    #[tokio::test]
    async fn test_store_and_read_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = AttachmentService::new(temp_dir.path().to_path_buf(), None);
        let mailbox_id = Uuid::new_v4();
        let data = b"Hello, attachment!";

        let (storage_path, checksum) = service
            .store_file(mailbox_id, data, "test.txt")
            .await
            .unwrap();

        // Added: Verify checksum is a valid SHA-256 hex string (64 chars)
        assert_eq!(checksum.len(), 64);
        assert!(storage_path.contains(&mailbox_id.to_string()));

        // Added: Verify file content round-trips correctly
        let read_data = service.read_file(&storage_path).await.unwrap();
        assert_eq!(read_data, data);
    }

    #[tokio::test]
    async fn test_store_file_creates_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = AttachmentService::new(temp_dir.path().to_path_buf(), None);
        let mailbox_id = Uuid::new_v4();

        // Added: Directory for this mailbox_id does not exist yet
        let mailbox_dir = temp_dir.path().join(mailbox_id.to_string());
        assert!(!mailbox_dir.exists());

        let (_path, _checksum) = service
            .store_file(mailbox_id, b"data", "file.txt")
            .await
            .unwrap();

        assert!(mailbox_dir.exists());
    }

    #[tokio::test]
    async fn test_delete_file_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = AttachmentService::new(temp_dir.path().to_path_buf(), None);
        let mailbox_id = Uuid::new_v4();

        let (storage_path, _) = service
            .store_file(mailbox_id, b"to delete", "delete-me.txt")
            .await
            .unwrap();

        service.delete_file(&storage_path).await.unwrap();

        // Added: Verify file is actually removed
        let result = service.read_file(&storage_path).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_file_not_found_is_ok() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = AttachmentService::new(temp_dir.path().to_path_buf(), None);

        // Added: Deleting a non-existent file should not error
        let result = service
            .delete_file("/nonexistent/path/file.txt")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_scan_file_no_clamav_configured() {
        let service = AttachmentService::new(PathBuf::from("/tmp"), None);

        let (status, result) = service.scan_file("/some/path").await.unwrap();
        assert_eq!(status, "clean");
        assert_eq!(result.unwrap(), "ClamAV not configured");
    }

    #[tokio::test]
    async fn test_scan_file_clamav_unavailable() {
        // Added: ClamAV configured but socket doesn't exist — should return error status gracefully
        let service = AttachmentService::new(
            PathBuf::from("/tmp"),
            Some("/nonexistent/clamav.sock".to_string()),
        );

        let (status, result) = service.scan_file("/some/path").await.unwrap();
        assert_eq!(status, "error");
        assert!(result.unwrap().contains("ClamAV unavailable"));
    }

    #[tokio::test]
    async fn test_checksum_deterministic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = AttachmentService::new(temp_dir.path().to_path_buf(), None);
        let mailbox_id = Uuid::new_v4();
        let data = b"deterministic content";

        let (_, checksum1) = service
            .store_file(mailbox_id, data, "file1.txt")
            .await
            .unwrap();
        let (_, checksum2) = service
            .store_file(mailbox_id, data, "file2.txt")
            .await
            .unwrap();

        // Added: Same content must produce same checksum for deduplication
        assert_eq!(checksum1, checksum2);
    }
}
