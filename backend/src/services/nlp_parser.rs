// Added: NLP query parser service for TMAIL-135
// PURPOSE: Parses natural language email search queries into structured IMAP search parameters via AI
// EXTERNAL: Uses ai_client for LLM API calls, returns ParsedSearchParams for IMAP search construction

use crate::models::ai_config::AiProvider;
use crate::models::nlp_search::ParsedSearchParams;
use crate::services::ai_client;

/// PURPOSE: Build the system prompt that instructs the AI how to parse natural language queries
/// CONSTRAINTS: The AI must return valid JSON matching ParsedSearchParams structure
pub fn format_ai_prompt(query: &str) -> String {
    format!(
        r#"You are an email search query parser. Given a natural language email search query, extract structured search parameters and return ONLY valid JSON with no other text.

The JSON must have this exact structure (omit fields that are not mentioned in the query):
{{
  "from": "sender email or name",
  "to": "recipient email or name",
  "subject": "subject keywords",
  "keywords": ["word1", "word2"],
  "date_from": "YYYY-MM-DD",
  "date_to": "YYYY-MM-DD",
  "folder": "INBOX or Sent or Drafts etc",
  "has_attachment": true/false
}}

Rules:
- For relative dates like "last week", "yesterday", "this month", calculate from today's date (2026-04-14)
- "last week" means date_from=7 days ago, date_to=today
- "yesterday" means date_from=date_to=yesterday's date
- "this month" means date_from=first of current month, date_to=today
- "last month" means the previous calendar month
- Extract sender/recipient names or emails from phrases like "from John", "to alice@example.com"
- Extract subject keywords from phrases like "about budget", "regarding the project"
- Extract general keywords from the remaining meaningful words
- Set has_attachment=true only when attachments are explicitly mentioned
- If a specific folder is mentioned (inbox, sent, drafts, trash, spam), include it
- Return ONLY the JSON object, no markdown formatting, no explanation

Query: {}"#,
        query
    )
}

/// PURPOSE: Parse a natural language query into structured search parameters using AI
/// EXTERNAL: Calls the user's configured AI provider via ai_client
pub async fn parse_natural_query(
    provider: &AiProvider,
    api_key: &str,
    model: &str,
    base_url: Option<&str>,
    query: &str,
) -> Result<ParsedSearchParams, String> {
    let prompt = format_ai_prompt(query);

    // Added: Call AI provider with low temperature for deterministic JSON output
    let response_text = ai_client::call_ai_provider(
        provider,
        api_key,
        model,
        base_url,
        &prompt,
        query,
        500,
        0.1,
    )
    .await?;

    // Added: Parse the AI response JSON into ParsedSearchParams
    parse_ai_response(&response_text)
}

/// PURPOSE: Parse the AI response text into ParsedSearchParams, handling markdown code fences
fn parse_ai_response(response: &str) -> Result<ParsedSearchParams, String> {
    // Added: Strip markdown code fences if the AI wrapped the JSON
    let cleaned = response.trim();
    let json_str = if cleaned.starts_with("```") {
        // NOTE: Remove opening ```json or ``` and closing ```
        let without_opening = cleaned
            .strip_prefix("```json")
            .or_else(|| cleaned.strip_prefix("```"))
            .unwrap_or(cleaned);
        without_opening
            .strip_suffix("```")
            .unwrap_or(without_opening)
            .trim()
    } else {
        cleaned
    };

    serde_json::from_str::<ParsedSearchParams>(json_str)
        .map_err(|err| format!("Failed to parse AI response as search params: {} — raw: {}", err, &response[..response.len().min(200)]))
}

/// PURPOSE: Convert parsed search parameters into an IMAP SEARCH command string
/// NOTE: IMAP SEARCH uses a specific syntax with criteria keywords
/// CONSTRAINTS: Date format must be "DD-Mon-YYYY" for IMAP (e.g., "14-Apr-2026")
pub fn build_imap_search(params: &ParsedSearchParams) -> String {
    let mut criteria: Vec<String> = Vec::new();

    // Added: FROM criterion
    if let Some(ref from) = params.from {
        criteria.push(format!("FROM \"{}\"", from));
    }

    // Added: TO criterion
    if let Some(ref to) = params.to {
        criteria.push(format!("TO \"{}\"", to));
    }

    // Added: SUBJECT criterion
    if let Some(ref subject) = params.subject {
        criteria.push(format!("SUBJECT \"{}\"", subject));
    }

    // Added: BODY keyword criteria — each keyword as a separate BODY search
    for keyword in &params.keywords {
        criteria.push(format!("BODY \"{}\"", keyword));
    }

    // Added: SINCE (date_from) criterion
    if let Some(ref date_from) = params.date_from {
        if let Some(imap_date) = iso_to_imap_date(date_from) {
            criteria.push(format!("SINCE {}", imap_date));
        }
    }

    // Added: BEFORE (date_to) criterion — IMAP BEFORE is exclusive, so add 1 day
    if let Some(ref date_to) = params.date_to {
        if let Some(imap_date) = iso_to_imap_date(date_to) {
            criteria.push(format!("BEFORE {}", imap_date));
        }
    }

    // Added: Attachment heuristic — search for Content-Disposition header
    if params.has_attachment == Some(true) {
        criteria.push("HEADER Content-Disposition attachment".to_string());
    }

    // NOTE: If no criteria were extracted, fall back to searching ALL
    if criteria.is_empty() {
        "ALL".to_string()
    } else {
        criteria.join(" ")
    }
}

/// PURPOSE: Convert an ISO date string (YYYY-MM-DD) to IMAP date format (DD-Mon-YYYY)
/// CONSTRAINTS: Returns None if the date string is not valid ISO format
fn iso_to_imap_date(iso_date: &str) -> Option<String> {
    let parts: Vec<&str> = iso_date.split('-').collect();
    if parts.len() != 3 {
        return None;
    }

    let year = parts[0];
    let month = parts[1];
    let day = parts[2];

    let month_name = match month {
        "01" => "Jan",
        "02" => "Feb",
        "03" => "Mar",
        "04" => "Apr",
        "05" => "May",
        "06" => "Jun",
        "07" => "Jul",
        "08" => "Aug",
        "09" => "Sep",
        "10" => "Oct",
        "11" => "Nov",
        "12" => "Dec",
        _ => return None,
    };

    // NOTE: IMAP date format is DD-Mon-YYYY (e.g., "14-Apr-2026")
    Some(format!("{}-{}-{}", day, month_name, year))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- build_imap_search tests --

    #[test]
    fn test_build_imap_search_empty_params() {
        let params = ParsedSearchParams::default();
        assert_eq!(build_imap_search(&params), "ALL");
    }

    #[test]
    fn test_build_imap_search_from_only() {
        let params = ParsedSearchParams {
            from: Some("john@example.com".to_string()),
            ..Default::default()
        };
        assert_eq!(build_imap_search(&params), "FROM \"john@example.com\"");
    }

    #[test]
    fn test_build_imap_search_to_only() {
        let params = ParsedSearchParams {
            to: Some("alice@example.com".to_string()),
            ..Default::default()
        };
        assert_eq!(build_imap_search(&params), "TO \"alice@example.com\"");
    }

    #[test]
    fn test_build_imap_search_subject_only() {
        let params = ParsedSearchParams {
            subject: Some("budget review".to_string()),
            ..Default::default()
        };
        assert_eq!(build_imap_search(&params), "SUBJECT \"budget review\"");
    }

    #[test]
    fn test_build_imap_search_keywords() {
        let params = ParsedSearchParams {
            keywords: vec!["quarterly".to_string(), "report".to_string()],
            ..Default::default()
        };
        assert_eq!(
            build_imap_search(&params),
            "BODY \"quarterly\" BODY \"report\""
        );
    }

    #[test]
    fn test_build_imap_search_date_range() {
        let params = ParsedSearchParams {
            date_from: Some("2025-03-01".to_string()),
            date_to: Some("2025-03-31".to_string()),
            ..Default::default()
        };
        assert_eq!(
            build_imap_search(&params),
            "SINCE 01-Mar-2025 BEFORE 31-Mar-2025"
        );
    }

    #[test]
    fn test_build_imap_search_has_attachment() {
        let params = ParsedSearchParams {
            has_attachment: Some(true),
            ..Default::default()
        };
        assert_eq!(
            build_imap_search(&params),
            "HEADER Content-Disposition attachment"
        );
    }

    #[test]
    fn test_build_imap_search_no_attachment_flag_false() {
        // NOTE: has_attachment=false should NOT add any attachment criterion
        let params = ParsedSearchParams {
            has_attachment: Some(false),
            ..Default::default()
        };
        assert_eq!(build_imap_search(&params), "ALL");
    }

    #[test]
    fn test_build_imap_search_combined_criteria() {
        let params = ParsedSearchParams {
            from: Some("john@example.com".to_string()),
            subject: Some("budget".to_string()),
            keywords: vec!["quarterly".to_string()],
            date_from: Some("2025-01-01".to_string()),
            has_attachment: Some(true),
            ..Default::default()
        };
        let result = build_imap_search(&params);
        assert!(result.contains("FROM \"john@example.com\""));
        assert!(result.contains("SUBJECT \"budget\""));
        assert!(result.contains("BODY \"quarterly\""));
        assert!(result.contains("SINCE 01-Jan-2025"));
        assert!(result.contains("HEADER Content-Disposition attachment"));
    }

    #[test]
    fn test_build_imap_search_full_params() {
        let params = ParsedSearchParams {
            from: Some("sender@example.com".to_string()),
            to: Some("recipient@example.com".to_string()),
            subject: Some("project update".to_string()),
            keywords: vec!["milestone".to_string()],
            date_from: Some("2025-06-01".to_string()),
            date_to: Some("2025-06-30".to_string()),
            folder: Some("INBOX".to_string()),
            has_attachment: Some(true),
        };
        let result = build_imap_search(&params);
        assert!(result.contains("FROM \"sender@example.com\""));
        assert!(result.contains("TO \"recipient@example.com\""));
        assert!(result.contains("SUBJECT \"project update\""));
        assert!(result.contains("BODY \"milestone\""));
        assert!(result.contains("SINCE 01-Jun-2025"));
        assert!(result.contains("BEFORE 30-Jun-2025"));
        assert!(result.contains("HEADER Content-Disposition attachment"));
        // NOTE: folder is used by the handler for IMAP mailbox selection, not in the SEARCH command
    }

    // -- iso_to_imap_date tests --

    #[test]
    fn test_iso_to_imap_date_valid() {
        assert_eq!(iso_to_imap_date("2025-04-14"), Some("14-Apr-2025".to_string()));
        assert_eq!(iso_to_imap_date("2025-01-01"), Some("01-Jan-2025".to_string()));
        assert_eq!(iso_to_imap_date("2025-12-31"), Some("31-Dec-2025".to_string()));
    }

    #[test]
    fn test_iso_to_imap_date_invalid_month() {
        assert_eq!(iso_to_imap_date("2025-13-01"), None);
        assert_eq!(iso_to_imap_date("2025-00-15"), None);
    }

    #[test]
    fn test_iso_to_imap_date_invalid_format() {
        assert_eq!(iso_to_imap_date("not-a-date"), None);
        assert_eq!(iso_to_imap_date("2025/04/14"), None);
        assert_eq!(iso_to_imap_date(""), None);
    }

    // -- parse_ai_response tests --

    #[test]
    fn test_parse_ai_response_clean_json() {
        let response = r#"{"from": "john@example.com", "subject": "budget", "keywords": ["review"]}"#;
        let params = parse_ai_response(response).unwrap();
        assert_eq!(params.from.unwrap(), "john@example.com");
        assert_eq!(params.subject.unwrap(), "budget");
        assert_eq!(params.keywords, vec!["review"]);
    }

    #[test]
    fn test_parse_ai_response_with_code_fences() {
        let response = "```json\n{\"from\": \"alice@example.com\", \"keywords\": []}\n```";
        let params = parse_ai_response(response).unwrap();
        assert_eq!(params.from.unwrap(), "alice@example.com");
    }

    #[test]
    fn test_parse_ai_response_with_bare_code_fences() {
        let response = "```\n{\"subject\": \"meeting\"}\n```";
        let params = parse_ai_response(response).unwrap();
        assert_eq!(params.subject.unwrap(), "meeting");
    }

    #[test]
    fn test_parse_ai_response_invalid_json() {
        let response = "This is not JSON at all";
        let result = parse_ai_response(response);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse"));
    }

    // -- format_ai_prompt tests --

    #[test]
    fn test_format_ai_prompt_contains_query() {
        let prompt = format_ai_prompt("emails from John about budget");
        assert!(prompt.contains("emails from John about budget"));
        assert!(prompt.contains("YYYY-MM-DD"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn test_format_ai_prompt_includes_structure_guidance() {
        let prompt = format_ai_prompt("test query");
        assert!(prompt.contains("from"));
        assert!(prompt.contains("to"));
        assert!(prompt.contains("subject"));
        assert!(prompt.contains("keywords"));
        assert!(prompt.contains("has_attachment"));
    }
}
