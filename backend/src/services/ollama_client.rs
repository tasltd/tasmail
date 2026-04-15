// Added: Ollama local LLM client service for TMAIL-102
// PURPOSE: Health checks, model listing, pulling, and deletion against an Ollama server
// EXTERNAL: Uses reqwest for HTTP calls to the Ollama REST API
// CONSTRAINTS: Ollama API docs at https://github.com/ollama/ollama/blob/main/docs/api.md

use crate::models::ollama_config::OllamaModelInfo;

/// PURPOSE: Result of a health check against the Ollama server
#[derive(Debug)]
pub struct OllamaHealthResult {
    pub running: bool,
    pub version: Option<String>,
}

/// PURPOSE: Result of a model pull operation
#[derive(Debug)]
pub struct PullResult {
    pub success: bool,
    pub message: String,
}

/// PURPOSE: Check if the Ollama server is running and get its version
/// EXTERNAL: GET {base_url}/api/version
pub async fn check_health(base_url: &str) -> OllamaHealthResult {
    let url = format!("{}/api/version", base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return OllamaHealthResult {
                running: false,
                version: None,
            };
        }
    };

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            // Added: Parse the version from JSON response
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let version = body["version"].as_str().map(|s| s.to_string());
            OllamaHealthResult {
                running: true,
                version,
            }
        }
        _ => OllamaHealthResult {
            running: false,
            version: None,
        },
    }
}

/// PURPOSE: List all models available on the Ollama server
/// EXTERNAL: GET {base_url}/api/tags
pub async fn list_models(base_url: &str) -> Result<Vec<OllamaModelInfo>, String> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Ollama returned status {}", resp.status()));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    // Added: Parse the models array from the /api/tags response
    let models = body["models"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|m| OllamaModelInfo {
            name: m["name"].as_str().unwrap_or("unknown").to_string(),
            size: m["size"].as_u64(),
            parameter_size: m["details"]["parameter_size"]
                .as_str()
                .map(|s| s.to_string()),
            quantization_level: m["details"]["quantization_level"]
                .as_str()
                .map(|s| s.to_string()),
            modified_at: m["modified_at"].as_str().map(|s| s.to_string()),
        })
        .collect();

    Ok(models)
}

/// PURPOSE: Pull (download) a model from the Ollama library
/// EXTERNAL: POST {base_url}/api/pull with stream: false
pub async fn pull_model(base_url: &str, model: &str) -> PullResult {
    let url = format!("{}/api/pull", base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return PullResult {
                success: false,
                message: format!("Failed to create HTTP client: {}", e),
            };
        }
    };

    let body = serde_json::json!({
        "name": model,
        "stream": false
    });

    match client.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            let resp_body: serde_json::Value = resp.json().await.unwrap_or_default();
            let status = resp_body["status"]
                .as_str()
                .unwrap_or("success")
                .to_string();
            PullResult {
                success: true,
                message: status,
            }
        }
        Ok(resp) => {
            let status = resp.status();
            let error_body = resp.text().await.unwrap_or_default();
            PullResult {
                success: false,
                message: format!("Ollama returned {}: {}", status, error_body),
            }
        }
        Err(e) => PullResult {
            success: false,
            message: format!("Failed to connect to Ollama: {}", e),
        },
    }
}

/// PURPOSE: Delete a model from the Ollama server
/// EXTERNAL: DELETE {base_url}/api/delete
pub async fn delete_model(base_url: &str, model: &str) -> Result<(), String> {
    let url = format!("{}/api/delete", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let body = serde_json::json!({ "name": model });

    let resp = client
        .delete(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let error_body = resp.text().await.unwrap_or_default();
        Err(format!("Ollama returned {}: {}", status, error_body))
    }
}

/// PURPOSE: Format byte size into human-readable string (e.g., "4.1 GB")
pub fn format_model_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_model_size_bytes() {
        assert_eq!(format_model_size(500), "500 B");
        assert_eq!(format_model_size(0), "0 B");
    }

    #[test]
    fn test_format_model_size_kilobytes() {
        assert_eq!(format_model_size(1024), "1.0 KB");
        assert_eq!(format_model_size(1536), "1.5 KB");
    }

    #[test]
    fn test_format_model_size_megabytes() {
        assert_eq!(format_model_size(1_048_576), "1.0 MB");
        assert_eq!(format_model_size(500 * 1024 * 1024), "500.0 MB");
    }

    #[test]
    fn test_format_model_size_gigabytes() {
        assert_eq!(format_model_size(1_073_741_824), "1.0 GB");
        // NOTE: 4.1 GB — typical size for a 7B param model
        assert_eq!(format_model_size(4_100_000_000), "3.8 GB");
        assert_eq!(format_model_size(7_400_000_000), "6.9 GB");
    }

    #[test]
    fn test_format_model_size_terabytes() {
        assert_eq!(format_model_size(1_099_511_627_776), "1.0 TB");
        assert_eq!(format_model_size(2_199_023_255_552), "2.0 TB");
    }

    #[test]
    fn test_format_model_size_boundary() {
        // NOTE: Exactly at the KB boundary
        assert_eq!(format_model_size(1023), "1023 B");
        assert_eq!(format_model_size(1024), "1.0 KB");
    }

    #[test]
    fn test_pull_result_success_fields() {
        let result = PullResult {
            success: true,
            message: "success".to_string(),
        };
        assert!(result.success);
        assert_eq!(result.message, "success");
    }

    #[test]
    fn test_pull_result_failure_fields() {
        let result = PullResult {
            success: false,
            message: "model not found".to_string(),
        };
        assert!(!result.success);
        assert!(result.message.contains("not found"));
    }

    #[test]
    fn test_health_result_running() {
        let health = OllamaHealthResult {
            running: true,
            version: Some("0.3.14".to_string()),
        };
        assert!(health.running);
        assert_eq!(health.version.as_deref(), Some("0.3.14"));
    }

    #[test]
    fn test_health_result_not_running() {
        let health = OllamaHealthResult {
            running: false,
            version: None,
        };
        assert!(!health.running);
        assert!(health.version.is_none());
    }

    #[test]
    fn test_format_model_size_large_values() {
        // Added: Test very large model sizes (405B param models can be 200+ GB)
        assert_eq!(format_model_size(230_000_000_000), "214.2 GB");
    }
}
