// Added: Rspamd HTTP API client service for TMAIL-15
// PURPOSE: Communicates with Rspamd via its HTTP API for message checking, learning, and statistics
// EXTERNAL: Uses reqwest for HTTP calls to Rspamd endpoints (/checkv2, /learnspam, /learnham, /stat)
// CONSTRAINTS: Rspamd HTTP API docs at https://rspamd.com/doc/architecture/protocol.html

use serde::{Deserialize, Serialize};

/// PURPOSE: Result of scanning a message through Rspamd /checkv2
#[derive(Debug, Serialize, Deserialize)]
pub struct SpamCheckResult {
    pub score: f64,
    pub required_score: f64,
    pub action: String,
    pub symbols: Vec<SpamSymbol>,
}

/// PURPOSE: Individual spam detection symbol with score and description
#[derive(Debug, Serialize, Deserialize)]
pub struct SpamSymbol {
    pub name: String,
    pub score: f64,
    pub description: Option<String>,
}

/// PURPOSE: Aggregated statistics from Rspamd /stat endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct RspamdStats {
    pub scanned: u64,
    pub learned: u64,
    pub spam_count: u64,
    pub ham_count: u64,
    pub actions: RspamdActionStats,
}

/// PURPOSE: Per-action counters from Rspamd statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct RspamdActionStats {
    pub reject: u64,
    pub greylist: u64,
    pub add_header: u64,
    pub no_action: u64,
}

/// PURPOSE: Rspamd HTTP API client wrapping reqwest calls
pub struct RspamdClient {
    pub base_url: String,
    pub password: Option<String>,
}

impl RspamdClient {
    pub fn new(base_url: String, password: Option<String>) -> Self {
        Self { base_url, password }
    }

    /// PURPOSE: Build an HTTP client with timeout and optional auth password header
    fn build_client(&self, timeout_secs: u64) -> Result<reqwest::Client, String> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))
    }

    /// PURPOSE: Add Rspamd password header if configured
    fn add_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref pw) = self.password {
            req.header("Password", pw.as_str())
        } else {
            req
        }
    }

    /// PURPOSE: Check a raw email message against Rspamd for spam scoring
    /// EXTERNAL: POST {base_url}/checkv2 with raw email body
    pub async fn check_message(&self, raw_email: &[u8]) -> Result<SpamCheckResult, String> {
        let url = format!("{}/checkv2", self.base_url.trim_end_matches('/'));
        let client = self.build_client(30)?;

        let req = client.post(&url).body(raw_email.to_vec());
        let req = self.add_auth(req);

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Failed to connect to Rspamd: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Rspamd returned {}: {}", status, body));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Rspamd response: {}", e))?;

        // Added: Parse the checkv2 response into our structured result
        let symbols = body
            .get("symbols")
            .and_then(|s| s.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(name, val)| SpamSymbol {
                        name: name.clone(),
                        score: val["score"].as_f64().unwrap_or(0.0),
                        description: val["description"].as_str().map(|s| s.to_string()),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(SpamCheckResult {
            score: body["score"].as_f64().unwrap_or(0.0),
            required_score: body["required_score"].as_f64().unwrap_or(15.0),
            action: body["action"].as_str().unwrap_or("no action").to_string(),
            symbols,
        })
    }

    /// PURPOSE: Train Rspamd to classify a message as spam
    /// EXTERNAL: POST {base_url}/learnspam with raw email body
    pub async fn learn_spam(&self, raw_email: &[u8]) -> Result<(), String> {
        let url = format!("{}/learnspam", self.base_url.trim_end_matches('/'));
        let client = self.build_client(30)?;

        let req = client.post(&url).body(raw_email.to_vec());
        let req = self.add_auth(req);

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Failed to connect to Rspamd: {}", e))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(format!("Rspamd learn spam failed {}: {}", status, body))
        }
    }

    /// PURPOSE: Train Rspamd to classify a message as ham (not spam)
    /// EXTERNAL: POST {base_url}/learnham with raw email body
    pub async fn learn_ham(&self, raw_email: &[u8]) -> Result<(), String> {
        let url = format!("{}/learnham", self.base_url.trim_end_matches('/'));
        let client = self.build_client(30)?;

        let req = client.post(&url).body(raw_email.to_vec());
        let req = self.add_auth(req);

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Failed to connect to Rspamd: {}", e))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(format!("Rspamd learn ham failed {}: {}", status, body))
        }
    }

    /// PURPOSE: Fetch aggregated statistics from Rspamd
    /// EXTERNAL: GET {base_url}/stat
    pub async fn get_stat(&self) -> Result<RspamdStats, String> {
        let url = format!("{}/stat", self.base_url.trim_end_matches('/'));
        let client = self.build_client(10)?;

        let req = client.get(&url);
        let req = self.add_auth(req);

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Failed to connect to Rspamd: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Rspamd returned {}: {}", status, body));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Rspamd stat response: {}", e))?;

        // Added: Parse /stat response into structured stats
        Ok(RspamdStats {
            scanned: body["scanned"].as_u64().unwrap_or(0),
            learned: body["learned"].as_u64().unwrap_or(0),
            spam_count: body["spam_count"].as_u64().unwrap_or(0),
            ham_count: body["ham_count"].as_u64().unwrap_or(0),
            actions: RspamdActionStats {
                reject: body["actions"]["reject"].as_u64().unwrap_or(0),
                greylist: body["actions"]["greylist"].as_u64().unwrap_or(0),
                add_header: body["actions"]["add header"].as_u64().unwrap_or(0),
                no_action: body["actions"]["no action"].as_u64().unwrap_or(0),
            },
        })
    }
}

/// PURPOSE: Format a spam score into a human-readable risk level
pub fn score_risk_level(score: f64) -> &'static str {
    if score >= 15.0 {
        "critical"
    } else if score >= 6.0 {
        "high"
    } else if score >= 4.0 {
        "medium"
    } else {
        "low"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spam_check_result_serialization() {
        let result = SpamCheckResult {
            score: 12.5,
            required_score: 15.0,
            action: "add header".to_string(),
            symbols: vec![
                SpamSymbol {
                    name: "BAYES_SPAM".to_string(),
                    score: 5.0,
                    description: Some("Bayesian spam probability".to_string()),
                },
            ],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"score\":12.5"));
        assert!(json.contains("BAYES_SPAM"));
    }

    #[test]
    fn test_rspamd_stats_serialization() {
        let stats = RspamdStats {
            scanned: 10000,
            learned: 500,
            spam_count: 1500,
            ham_count: 8500,
            actions: RspamdActionStats {
                reject: 200,
                greylist: 300,
                add_header: 1000,
                no_action: 8500,
            },
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"scanned\":10000"));
        assert!(json.contains("\"reject\":200"));
    }

    #[test]
    fn test_rspamd_client_new() {
        let client = RspamdClient::new(
            "http://localhost:11333".to_string(),
            Some("secret".to_string()),
        );
        assert_eq!(client.base_url, "http://localhost:11333");
        assert_eq!(client.password, Some("secret".to_string()));
    }

    #[test]
    fn test_rspamd_client_no_password() {
        let client = RspamdClient::new("http://rspamd:11333".to_string(), None);
        assert!(client.password.is_none());
    }

    #[test]
    fn test_score_risk_level() {
        assert_eq!(score_risk_level(0.5), "low");
        assert_eq!(score_risk_level(3.9), "low");
        assert_eq!(score_risk_level(4.0), "medium");
        assert_eq!(score_risk_level(5.9), "medium");
        assert_eq!(score_risk_level(6.0), "high");
        assert_eq!(score_risk_level(14.9), "high");
        assert_eq!(score_risk_level(15.0), "critical");
        assert_eq!(score_risk_level(25.0), "critical");
    }

    #[test]
    fn test_spam_symbol_fields() {
        let sym = SpamSymbol {
            name: "DKIM_SIGNED".to_string(),
            score: -1.0,
            description: Some("DKIM signature verified".to_string()),
        };
        assert_eq!(sym.name, "DKIM_SIGNED");
        assert!(sym.score < 0.0);
    }

    #[test]
    fn test_action_stats_defaults() {
        let stats = RspamdActionStats {
            reject: 0,
            greylist: 0,
            add_header: 0,
            no_action: 0,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["reject"], 0);
    }

    #[test]
    fn test_spam_check_result_empty_symbols() {
        let result = SpamCheckResult {
            score: 0.0,
            required_score: 15.0,
            action: "no action".to_string(),
            symbols: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"symbols\":[]"));
    }

    #[test]
    fn test_rspamd_stats_deserialization() {
        let json = r#"{
            "scanned": 5000,
            "learned": 100,
            "spam_count": 500,
            "ham_count": 4500,
            "actions": {
                "reject": 50,
                "greylist": 100,
                "add_header": 350,
                "no_action": 4500
            }
        }"#;
        let stats: RspamdStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.scanned, 5000);
        assert_eq!(stats.actions.reject, 50);
    }
}
