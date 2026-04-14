// Added: AI API client abstraction for BYOK AI integration (TMAIL-105)
// PURPOSE: Formats and sends requests to various AI providers (OpenAI, Anthropic, Google, Ollama, custom)
// EXTERNAL: Uses reqwest for HTTP calls to AI provider APIs
// CONSTRAINTS: Each provider has different request/response formats

use crate::models::ai_config::AiProvider;

/// PURPOSE: Format a request body for OpenAI-compatible APIs
/// EXTERNAL: https://platform.openai.com/docs/api-reference/chat/create
pub fn format_openai_request(
    model: &str,
    system_prompt: &str,
    user_message: &str,
    max_tokens: i32,
    temperature: f32,
) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_message }
        ],
        "max_tokens": max_tokens,
        "temperature": temperature
    })
}

/// PURPOSE: Format a request body for the Anthropic Messages API
/// EXTERNAL: https://docs.anthropic.com/en/docs/build-with-claude/overview
pub fn format_anthropic_request(
    model: &str,
    system_prompt: &str,
    user_message: &str,
    max_tokens: i32,
    temperature: f32,
) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "temperature": temperature,
        "system": system_prompt,
        "messages": [
            { "role": "user", "content": user_message }
        ]
    })
}

/// PURPOSE: Format a request body for Google Gemini API
/// EXTERNAL: https://ai.google.dev/gemini-api/docs
pub fn format_google_request(
    _model: &str,
    system_prompt: &str,
    user_message: &str,
    max_tokens: i32,
    temperature: f32,
) -> serde_json::Value {
    serde_json::json!({
        "contents": [
            {
                "parts": [
                    { "text": format!("{}\n\n{}", system_prompt, user_message) }
                ]
            }
        ],
        "generationConfig": {
            "maxOutputTokens": max_tokens,
            "temperature": temperature
        }
    })
}

/// PURPOSE: Format a request body for Ollama generate API
/// EXTERNAL: https://github.com/ollama/ollama/blob/main/docs/api.md
pub fn format_ollama_request(
    model: &str,
    system_prompt: &str,
    user_message: &str,
    max_tokens: i32,
    temperature: f32,
) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "system": system_prompt,
        "prompt": user_message,
        "stream": false,
        "options": {
            "num_predict": max_tokens,
            "temperature": temperature
        }
    })
}

/// PURPOSE: Build the API URL for the given provider
/// NOTE: Google API appends the model and key to the URL path
pub fn build_api_url(provider: &AiProvider, base_url: Option<&str>, model: &str, api_key: &str) -> String {
    match provider {
        AiProvider::Openai | AiProvider::Custom => {
            let base = base_url.unwrap_or("https://api.openai.com/v1");
            format!("{}/chat/completions", base)
        }
        AiProvider::Anthropic => {
            let base = base_url.unwrap_or("https://api.anthropic.com/v1");
            format!("{}/messages", base)
        }
        AiProvider::Google => {
            let base = base_url.unwrap_or("https://generativelanguage.googleapis.com/v1beta");
            format!("{}/models/{}:generateContent?key={}", base, model, api_key)
        }
        AiProvider::Ollama => {
            let base = base_url.unwrap_or("http://localhost:11434");
            format!("{}/api/generate", base)
        }
    }
}

/// PURPOSE: Send a completion request to the configured AI provider and extract the response text
/// CONSTRAINTS: Uses 30s timeout to avoid blocking on slow APIs
/// EXTERNAL: Makes HTTP POST to the provider's API endpoint
pub async fn call_ai_provider(
    provider: &AiProvider,
    api_key: &str,
    model: &str,
    base_url: Option<&str>,
    system_prompt: &str,
    user_message: &str,
    max_tokens: i32,
    temperature: f32,
) -> Result<String, String> {
    let url = build_api_url(provider, base_url, model, api_key);
    let body = match provider {
        AiProvider::Openai | AiProvider::Custom => {
            format_openai_request(model, system_prompt, user_message, max_tokens, temperature)
        }
        AiProvider::Anthropic => {
            format_anthropic_request(model, system_prompt, user_message, max_tokens, temperature)
        }
        AiProvider::Google => {
            format_google_request(model, system_prompt, user_message, max_tokens, temperature)
        }
        AiProvider::Ollama => {
            format_ollama_request(model, system_prompt, user_message, max_tokens, temperature)
        }
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {}", err))?;

    let mut request_builder = client
        .post(&url)
        .header("Content-Type", "application/json");

    // Added: Set provider-specific auth headers
    match provider {
        AiProvider::Openai | AiProvider::Custom => {
            request_builder = request_builder.header("Authorization", format!("Bearer {}", api_key));
        }
        AiProvider::Anthropic => {
            request_builder = request_builder
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01");
        }
        AiProvider::Google => {
            // NOTE: Google uses API key in the URL query param, no auth header needed
        }
        AiProvider::Ollama => {
            // NOTE: Ollama is typically local and doesn't require auth
        }
    }

    let response = request_builder
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("AI API request failed: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        return Err(format!(
            "AI API returned status {}: {}",
            status,
            error_body.chars().take(500).collect::<String>()
        ));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Failed to parse AI API response: {}", err))?;

    // Added: Extract text from provider-specific response format
    extract_response_text(provider, &response_json)
}

/// PURPOSE: Extract the generated text from the provider-specific response JSON
fn extract_response_text(
    provider: &AiProvider,
    response: &serde_json::Value,
) -> Result<String, String> {
    match provider {
        AiProvider::Openai | AiProvider::Custom => {
            response["choices"][0]["message"]["content"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "Failed to extract text from OpenAI response — expected choices[0].message.content".to_string())
        }
        AiProvider::Anthropic => {
            response["content"][0]["text"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "Failed to extract text from Anthropic response — expected content[0].text".to_string())
        }
        AiProvider::Google => {
            response["candidates"][0]["content"]["parts"][0]["text"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "Failed to extract text from Google response — expected candidates[0].content.parts[0].text".to_string())
        }
        AiProvider::Ollama => {
            response["response"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "Failed to extract text from Ollama response — expected response field".to_string())
        }
    }
}

/// PURPOSE: Summarize an email using the user's AI config
pub async fn summarize_email(
    provider: &AiProvider,
    api_key: &str,
    model: &str,
    base_url: Option<&str>,
    max_tokens: i32,
    temperature: f32,
    email_text: &str,
) -> Result<String, String> {
    let system_prompt = "You are an email assistant. Summarize the following email concisely in 2-3 sentences. Focus on the key points, action items, and any deadlines mentioned.";
    call_ai_provider(
        provider, api_key, model, base_url, system_prompt, email_text, max_tokens, temperature,
    )
    .await
}

// Added: Thread/conversation summarization for TMAIL-103
/// PURPOSE: Summarize an email thread by concatenating multiple email texts and producing a combined summary
/// CONSTRAINTS: emails vec must not be empty; each entry is separated by "---" for clarity to the AI model
pub async fn summarize_thread(
    provider: &AiProvider,
    api_key: &str,
    model: &str,
    base_url: Option<&str>,
    max_tokens: i32,
    temperature: f32,
    emails: &[String],
) -> Result<String, String> {
    if emails.is_empty() {
        return Err("No emails provided for thread summarization".to_string());
    }
    // Added: Join all email texts with separator for the AI to understand thread boundaries
    let combined_text = emails.join("\n\n---\n\n");
    let system_prompt = "You are an email assistant. Summarize the following email conversation thread concisely. Identify the key topics discussed, decisions made, action items, and any open questions. Present the summary as a cohesive overview of the thread.";
    call_ai_provider(
        provider, api_key, model, base_url, system_prompt, &combined_text, max_tokens, temperature,
    )
    .await
}

/// PURPOSE: Generate reply suggestions for an email with tone control (TMAIL-104)
/// CONSTRAINTS: tone must be one of "brief", "detailed", or "decline"
pub async fn suggest_reply(
    provider: &AiProvider,
    api_key: &str,
    model: &str,
    base_url: Option<&str>,
    max_tokens: i32,
    temperature: f32,
    email_text: &str,
    tone: &str,
) -> Result<String, String> {
    // Added: Tone-specific system prompts for smart reply generation
    let system_prompt = match tone {
        "brief" => "You are an email assistant. Write a brief, professional reply to the following email. Keep it to 2-3 sentences maximum. Be polite and to the point.",
        "detailed" => "You are an email assistant. Write a thorough, detailed reply to the following email. Address all points raised, provide relevant context, and be comprehensive while remaining professional.",
        "decline" => "You are an email assistant. Write a polite, professional reply declining or respectfully saying no to the request in the following email. Be gracious but firm, and suggest alternatives if appropriate.",
        _ => "You are an email assistant. Generate a professional, concise reply to the following email. Keep it brief and to the point.",
    };
    call_ai_provider(
        provider, api_key, model, base_url, system_prompt, email_text, max_tokens, temperature,
    )
    .await
}

/// PURPOSE: Map SmartReplyTone enum to string for prompt selection (TMAIL-104)
pub fn tone_to_str(tone: &crate::models::ai_config::SmartReplyTone) -> &'static str {
    use crate::models::ai_config::SmartReplyTone;
    match tone {
        SmartReplyTone::Brief => "brief",
        SmartReplyTone::Detailed => "detailed",
        SmartReplyTone::Decline => "decline",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_openai_request_structure() {
        let payload = format_openai_request("gpt-4o", "You are helpful.", "Hello", 500, 0.5);
        assert_eq!(payload["model"], "gpt-4o");
        let messages = payload["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "You are helpful.");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "Hello");
        assert_eq!(payload["max_tokens"], 500);
        // NOTE: f32 precision — check temperature is present and within range
        let temp = payload["temperature"].as_f64().unwrap();
        assert!((temp - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_format_anthropic_request_structure() {
        let payload = format_anthropic_request("claude-sonnet-4-20250514", "System prompt", "User msg", 1000, 0.5);
        assert_eq!(payload["model"], "claude-sonnet-4-20250514");
        assert_eq!(payload["system"], "System prompt");
        assert_eq!(payload["max_tokens"], 1000);
        assert_eq!(payload["temperature"], 0.5);
        let messages = payload["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "User msg");
    }

    #[test]
    fn test_format_google_request_structure() {
        let payload = format_google_request("gemini-2.5-pro", "System", "User", 300, 0.5);
        let contents = payload["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        let text = contents[0]["parts"][0]["text"].as_str().unwrap();
        assert!(text.contains("System"));
        assert!(text.contains("User"));
        assert_eq!(payload["generationConfig"]["maxOutputTokens"], 300);
        let temp = payload["generationConfig"]["temperature"].as_f64().unwrap();
        assert!((temp - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_format_ollama_request_structure() {
        let payload = format_ollama_request("llama3", "System", "Prompt", 200, 0.5);
        assert_eq!(payload["model"], "llama3");
        assert_eq!(payload["system"], "System");
        assert_eq!(payload["prompt"], "Prompt");
        assert_eq!(payload["stream"], false);
        assert_eq!(payload["options"]["num_predict"], 200);
        let temp = payload["options"]["temperature"].as_f64().unwrap();
        assert!((temp - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_build_api_url_openai_default() {
        let url = build_api_url(&AiProvider::Openai, None, "gpt-4o", "sk-test");
        assert_eq!(url, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn test_build_api_url_openai_custom_base() {
        let url = build_api_url(&AiProvider::Openai, Some("https://my-proxy.example.com/v1"), "gpt-4o", "sk-test");
        assert_eq!(url, "https://my-proxy.example.com/v1/chat/completions");
    }

    #[test]
    fn test_build_api_url_anthropic_default() {
        let url = build_api_url(&AiProvider::Anthropic, None, "claude-sonnet-4-20250514", "key");
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn test_build_api_url_google_includes_key_and_model() {
        let url = build_api_url(&AiProvider::Google, None, "gemini-2.5-pro", "my-api-key");
        assert!(url.contains("gemini-2.5-pro"));
        assert!(url.contains("my-api-key"));
        assert!(url.contains("generateContent"));
    }

    #[test]
    fn test_build_api_url_ollama_default() {
        let url = build_api_url(&AiProvider::Ollama, None, "llama3", "");
        assert_eq!(url, "http://localhost:11434/api/generate");
    }

    #[test]
    fn test_build_api_url_ollama_custom_host() {
        let url = build_api_url(&AiProvider::Ollama, Some("http://gpu-server:11434"), "llama3", "");
        assert_eq!(url, "http://gpu-server:11434/api/generate");
    }

    #[test]
    fn test_build_api_url_custom_provider() {
        let url = build_api_url(&AiProvider::Custom, Some("https://my-llm.example.com/api"), "custom-model", "key");
        assert_eq!(url, "https://my-llm.example.com/api/chat/completions");
    }

    #[test]
    fn test_extract_response_text_openai() {
        let response = serde_json::json!({
            "choices": [{
                "message": { "content": "This is the summary." }
            }]
        });
        let text = extract_response_text(&AiProvider::Openai, &response).unwrap();
        assert_eq!(text, "This is the summary.");
    }

    #[test]
    fn test_extract_response_text_anthropic() {
        let response = serde_json::json!({
            "content": [{
                "type": "text",
                "text": "Anthropic response text."
            }]
        });
        let text = extract_response_text(&AiProvider::Anthropic, &response).unwrap();
        assert_eq!(text, "Anthropic response text.");
    }

    #[test]
    fn test_extract_response_text_google() {
        let response = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": "Google response." }]
                }
            }]
        });
        let text = extract_response_text(&AiProvider::Google, &response).unwrap();
        assert_eq!(text, "Google response.");
    }

    #[test]
    fn test_extract_response_text_ollama() {
        let response = serde_json::json!({
            "response": "Ollama generated text."
        });
        let text = extract_response_text(&AiProvider::Ollama, &response).unwrap();
        assert_eq!(text, "Ollama generated text.");
    }

    #[test]
    fn test_extract_response_text_openai_missing_content() {
        let response = serde_json::json!({ "choices": [] });
        let result = extract_response_text(&AiProvider::Openai, &response);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OpenAI"));
    }

    #[test]
    fn test_extract_response_text_anthropic_missing_content() {
        let response = serde_json::json!({ "content": [] });
        let result = extract_response_text(&AiProvider::Anthropic, &response);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Anthropic"));
    }

    #[test]
    fn test_extract_response_text_google_missing_candidates() {
        let response = serde_json::json!({ "candidates": [] });
        let result = extract_response_text(&AiProvider::Google, &response);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_response_text_ollama_missing_response() {
        let response = serde_json::json!({ "done": true });
        let result = extract_response_text(&AiProvider::Ollama, &response);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_response_text_custom_uses_openai_format() {
        // Added: Custom provider should use OpenAI response format
        let response = serde_json::json!({
            "choices": [{
                "message": { "content": "Custom provider response." }
            }]
        });
        let text = extract_response_text(&AiProvider::Custom, &response).unwrap();
        assert_eq!(text, "Custom provider response.");
    }

    // Added: Tests for tone_to_str helper (TMAIL-104)
    #[test]
    fn test_tone_to_str_brief() {
        use crate::models::ai_config::SmartReplyTone;
        assert_eq!(tone_to_str(&SmartReplyTone::Brief), "brief");
    }

    #[test]
    fn test_tone_to_str_detailed() {
        use crate::models::ai_config::SmartReplyTone;
        assert_eq!(tone_to_str(&SmartReplyTone::Detailed), "detailed");
    }

    #[test]
    fn test_tone_to_str_decline() {
        use crate::models::ai_config::SmartReplyTone;
        assert_eq!(tone_to_str(&SmartReplyTone::Decline), "decline");
    }

    // Added: Test for summarize_thread empty emails validation (TMAIL-103)
    #[tokio::test]
    async fn test_summarize_thread_empty_emails_returns_error() {
        let result = summarize_thread(
            &AiProvider::Openai,
            "sk-test",
            "gpt-4o",
            None,
            500,
            0.7,
            &[],
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No emails provided"));
    }
}
