use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Added: Email template for reusable message composition
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailTemplate {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub name: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    /// Merge field names stored as JSONB (e.g. ["first_name", "company"])
    pub merge_fields: serde_json::Value,
    pub category: Option<String>,
    /// Whether this template is visible to all users in the domain
    pub is_shared: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplate {
    pub name: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    pub merge_fields: Option<Vec<String>>,
    pub category: Option<String>,
    pub is_shared: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTemplate {
    pub name: Option<String>,
    pub subject: Option<String>,
    pub body_html: Option<String>,
    pub body_text: Option<String>,
    pub merge_fields: Option<Vec<String>>,
    pub category: Option<String>,
    pub is_shared: Option<bool>,
}

/// Request body for rendering a template with merge field values
#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    pub fields: HashMap<String, String>,
}

/// Rendered template output with placeholders replaced
#[derive(Debug, Serialize)]
pub struct RenderResult {
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
}

impl EmailTemplate {
    pub async fn find_by_mailbox(pool: &PgPool, mailbox_id: Uuid) -> Result<Vec<EmailTemplate>, sqlx::Error> {
        sqlx::query_as::<_, EmailTemplate>(
            "SELECT * FROM email_templates WHERE mailbox_id = $1 ORDER BY name ASC, created_at ASC"
        )
        .bind(mailbox_id)
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<Option<EmailTemplate>, sqlx::Error> {
        sqlx::query_as::<_, EmailTemplate>(
            "SELECT * FROM email_templates WHERE id = $1 AND mailbox_id = $2"
        )
        .bind(id)
        .bind(mailbox_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        pool: &PgPool,
        mailbox_id: Uuid,
        data: &CreateTemplate,
    ) -> Result<EmailTemplate, sqlx::Error> {
        let merge_fields_json = match &data.merge_fields {
            Some(fields) => serde_json::to_value(fields).unwrap_or_default(),
            None => serde_json::Value::Array(vec![]),
        };

        sqlx::query_as::<_, EmailTemplate>(
            "INSERT INTO email_templates (mailbox_id, name, subject, body_html, body_text, merge_fields, category, is_shared)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING *"
        )
        .bind(mailbox_id)
        .bind(&data.name)
        .bind(&data.subject)
        .bind(&data.body_html)
        .bind(&data.body_text)
        .bind(merge_fields_json)
        .bind(&data.category)
        .bind(data.is_shared.unwrap_or(false))
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        mailbox_id: Uuid,
        data: &UpdateTemplate,
    ) -> Result<Option<EmailTemplate>, sqlx::Error> {
        // NOTE: partial update — only provided fields are changed
        let existing = Self::find_by_id(pool, id, mailbox_id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let name = data.name.as_deref().unwrap_or(&existing.name);
        let subject = data.subject.as_deref().unwrap_or(&existing.subject);
        let body_html = data.body_html.as_deref().unwrap_or(&existing.body_html);
        let body_text = data.body_text.as_deref().unwrap_or(&existing.body_text);
        let category = data.category.as_deref().or(existing.category.as_deref());
        let is_shared = data.is_shared.unwrap_or(existing.is_shared);

        let merge_fields_json = match &data.merge_fields {
            Some(fields) => serde_json::to_value(fields).unwrap_or_default(),
            None => existing.merge_fields.clone(),
        };

        sqlx::query_as::<_, EmailTemplate>(
            "UPDATE email_templates
             SET name = $3, subject = $4, body_html = $5, body_text = $6,
                 merge_fields = $7, category = $8, is_shared = $9, updated_at = NOW()
             WHERE id = $1 AND mailbox_id = $2
             RETURNING *"
        )
        .bind(id)
        .bind(mailbox_id)
        .bind(name)
        .bind(subject)
        .bind(body_html)
        .bind(body_text)
        .bind(merge_fields_json)
        .bind(category)
        .bind(is_shared)
        .fetch_optional(pool)
        .await
    }

    pub async fn delete(pool: &PgPool, id: Uuid, mailbox_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM email_templates WHERE id = $1 AND mailbox_id = $2"
        )
        .bind(id)
        .bind(mailbox_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Render this template by replacing {{field_name}} placeholders with provided values
    pub fn render(&self, fields: &HashMap<String, String>) -> RenderResult {
        let mut rendered_subject = self.subject.clone();
        let mut rendered_html = self.body_html.clone();
        let mut rendered_text = self.body_text.clone();

        for (key, value) in fields {
            let placeholder = format!("{{{{{}}}}}", key);
            rendered_subject = rendered_subject.replace(&placeholder, value);
            rendered_html = rendered_html.replace(&placeholder, value);
            rendered_text = rendered_text.replace(&placeholder, value);
        }

        RenderResult {
            subject: rendered_subject,
            body_html: rendered_html,
            body_text: rendered_text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_template(subject: &str, body_html: &str, body_text: &str) -> EmailTemplate {
        EmailTemplate {
            id: Uuid::new_v4(),
            mailbox_id: Uuid::new_v4(),
            name: "Test Template".to_string(),
            subject: subject.to_string(),
            body_html: body_html.to_string(),
            body_text: body_text.to_string(),
            merge_fields: serde_json::to_value(vec!["first_name", "company"]).unwrap(),
            category: Some("Business".to_string()),
            is_shared: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_render_replaces_placeholders() {
        let template = make_template(
            "Hello {{first_name}}",
            "<p>Welcome to {{company}}, {{first_name}}!</p>",
            "Welcome to {{company}}, {{first_name}}!",
        );

        let mut fields = HashMap::new();
        fields.insert("first_name".to_string(), "Alice".to_string());
        fields.insert("company".to_string(), "Acme Corp".to_string());

        let result = template.render(&fields);
        assert_eq!(result.subject, "Hello Alice");
        assert_eq!(result.body_html, "<p>Welcome to Acme Corp, Alice!</p>");
        assert_eq!(result.body_text, "Welcome to Acme Corp, Alice!");
    }

    #[test]
    fn test_render_with_empty_fields() {
        let template = make_template(
            "Subject {{name}}",
            "<p>Hi {{name}}</p>",
            "Hi {{name}}",
        );
        let fields = HashMap::new();

        let result = template.render(&fields);
        // NOTE: Unreplaced placeholders remain as-is
        assert_eq!(result.subject, "Subject {{name}}");
        assert_eq!(result.body_html, "<p>Hi {{name}}</p>");
    }

    #[test]
    fn test_render_with_no_placeholders() {
        let template = make_template(
            "Plain subject",
            "<p>No placeholders here</p>",
            "No placeholders here",
        );
        let mut fields = HashMap::new();
        fields.insert("unused".to_string(), "value".to_string());

        let result = template.render(&fields);
        assert_eq!(result.subject, "Plain subject");
        assert_eq!(result.body_html, "<p>No placeholders here</p>");
    }

    #[test]
    fn test_render_multiple_occurrences() {
        let template = make_template(
            "{{name}} - {{name}}",
            "<p>{{name}} and {{name}}</p>",
            "{{name}} and {{name}}",
        );
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "Bob".to_string());

        let result = template.render(&fields);
        assert_eq!(result.subject, "Bob - Bob");
        assert_eq!(result.body_html, "<p>Bob and Bob</p>");
        assert_eq!(result.body_text, "Bob and Bob");
    }

    #[test]
    fn test_create_request_deserialization() {
        let json = r#"{
            "name": "Welcome Email",
            "subject": "Welcome {{first_name}}!",
            "body_html": "<h1>Hello {{first_name}}</h1>",
            "body_text": "Hello {{first_name}}",
            "merge_fields": ["first_name", "company"],
            "category": "Onboarding",
            "is_shared": true
        }"#;
        let req: CreateTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Welcome Email");
        assert_eq!(req.merge_fields.as_ref().unwrap().len(), 2);
        assert_eq!(req.category.as_deref(), Some("Onboarding"));
        assert_eq!(req.is_shared, Some(true));
    }

    #[test]
    fn test_create_request_minimal() {
        let json = r#"{
            "name": "Quick Reply",
            "subject": "Re: your message",
            "body_html": "<p>Thanks!</p>",
            "body_text": "Thanks!"
        }"#;
        let req: CreateTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "Quick Reply");
        assert!(req.merge_fields.is_none());
        assert!(req.category.is_none());
        assert!(req.is_shared.is_none());
    }

    #[test]
    fn test_update_request_partial() {
        let json = r#"{"name": "Updated Name"}"#;
        let req: UpdateTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(req.name.as_deref(), Some("Updated Name"));
        assert!(req.subject.is_none());
        assert!(req.body_html.is_none());
        assert!(req.merge_fields.is_none());
    }

    #[test]
    fn test_template_serialization_roundtrip() {
        let template = make_template("Sub", "<p>Body</p>", "Body");
        let json = serde_json::to_string(&template).unwrap();
        let parsed: EmailTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, template.name);
        assert_eq!(parsed.subject, template.subject);
        assert_eq!(parsed.is_shared, template.is_shared);
        assert_eq!(parsed.category, template.category);
    }

    #[test]
    fn test_render_request_deserialization() {
        let json = r#"{"fields": {"first_name": "Alice", "company": "Acme"}}"#;
        let req: RenderRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.fields.len(), 2);
        assert_eq!(req.fields.get("first_name").unwrap(), "Alice");
    }

    #[test]
    fn test_render_result_serialization() {
        let result = RenderResult {
            subject: "Hello Alice".to_string(),
            body_html: "<p>Welcome</p>".to_string(),
            body_text: "Welcome".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Hello Alice"));
        assert!(json.contains("<p>Welcome</p>"));
    }
}
