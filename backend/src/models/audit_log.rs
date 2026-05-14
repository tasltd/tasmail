use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: Uuid,
    pub mailbox_id: Option<Uuid>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl AuditLog {
    /// Record an audit log entry.
    ///
    /// Fix: TMAIL-198 — pin app.is_admin = true on the recording connection
    /// so the audit_log_admin RLS policy permits the insert. Without this
    /// every record() call silently failed (the table's RLS policies require
    /// either matching mailbox_id or admin context, and most callers run
    /// with neither set on the freshly-acquired pool connection). Treating
    /// audit recording as a system-internal write — every successful auth
    /// or admin action should be logged regardless of the current request's
    /// session vars.
    pub async fn record(
        pool: &PgPool,
        mailbox_id: Option<Uuid>,
        action: &str,
        resource_type: Option<&str>,
        resource_id: Option<&str>,
        details: Option<serde_json::Value>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;
        sqlx::query("SELECT set_config('app.is_admin', 'true', false)")
            .execute(&mut *conn)
            .await?;
        sqlx::query(
            "INSERT INTO audit_log (mailbox_id, action, resource_type, resource_id, details, ip_address, user_agent) VALUES ($1, $2, $3, $4, $5, $6, $7)"
        )
        .bind(mailbox_id)
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(details)
        .bind(ip_address)
        .bind(user_agent)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Build the SQL + bind params for a filtered query. Shared by query()
    /// and query_with_conn() so the connection-pinned admin viewer (TMAIL-198)
    /// reuses the exact same predicate logic as the historical pool variant.
    ///
    /// Action filter semantics: an action ending in `.` is treated as a
    /// prefix match (e.g. `auth.` matches `auth.login`, `auth.signup`).
    /// Exact strings (e.g. `auth.login`) match only that action.
    fn build_filtered_query(
        mailbox_id: Option<Uuid>,
        action: Option<&str>,
        limit: i64,
    ) -> (String, Vec<String>) {
        let mut query = String::from("SELECT * FROM audit_log WHERE 1=1");
        let mut params: Vec<String> = Vec::new();

        if let Some(mid) = mailbox_id {
            params.push(mid.to_string());
            query.push_str(&format!(" AND mailbox_id = ${}", params.len()));
        }
        if let Some(act) = action {
            if act.ends_with('.') {
                // Prefix match — `auth.` → `auth.%`
                params.push(format!("{}%", act));
                query.push_str(&format!(" AND action LIKE ${}", params.len()));
            } else {
                params.push(act.to_string());
                query.push_str(&format!(" AND action = ${}", params.len()));
            }
        }

        query.push_str(" ORDER BY created_at DESC");
        query.push_str(&format!(" LIMIT {}", limit));
        (query, params)
    }

    /// Query audit log with optional filters (uses bare pool — kept for the
    /// non-RLS-sensitive call sites).
    pub async fn query(
        pool: &PgPool,
        mailbox_id: Option<Uuid>,
        action: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AuditLog>, sqlx::Error> {
        let (query, params) = Self::build_filtered_query(mailbox_id, action, limit);
        let mut q = sqlx::query_as::<_, AuditLog>(&query);
        for param in &params {
            q = q.bind(param);
        }
        q.fetch_all(pool).await
    }

    /// Same as query() but runs against a caller-supplied connection so
    /// previously-set RLS session vars (app.is_admin) carry through. Used
    /// by the admin viewer (TMAIL-198) where the audit_log table is empty
    /// without app.is_admin = 'true'.
    pub async fn query_with_conn(
        conn: &mut sqlx::PgConnection,
        mailbox_id: Option<Uuid>,
        action: Option<&str>,
        limit: i64,
    ) -> Result<Vec<AuditLog>, sqlx::Error> {
        let (query, params) = Self::build_filtered_query(mailbox_id, action, limit);
        let mut q = sqlx::query_as::<_, AuditLog>(&query);
        for param in &params {
            q = q.bind(param);
        }
        q.fetch_all(conn).await
    }

    /// Build the query string for testing/debugging purposes
    fn build_query(mailbox_id: Option<Uuid>, action: Option<&str>, limit: i64) -> (String, Vec<String>) {
        let mut query = String::from("SELECT * FROM audit_log WHERE 1=1");
        let mut params: Vec<String> = Vec::new();

        if let Some(mid) = mailbox_id {
            params.push(mid.to_string());
            query.push_str(&format!(" AND mailbox_id = ${}", params.len()));
        }
        if let Some(act) = action {
            params.push(act.to_string());
            query.push_str(&format!(" AND action = ${}", params.len()));
        }

        query.push_str(" ORDER BY created_at DESC");
        query.push_str(&format!(" LIMIT {}", limit));

        (query, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_serialization() {
        let log = AuditLog {
            id: Uuid::new_v4(),
            mailbox_id: Some(Uuid::new_v4()),
            action: "auth.login".to_string(),
            resource_type: Some("session".to_string()),
            resource_id: None,
            details: Some(serde_json::json!({"username": "test@example.com"})),
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
            created_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("auth.login"));
        assert!(json.contains("session"));
        assert!(json.contains("192.168.1.1"));

        let deserialized: AuditLog = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.action, "auth.login");
        assert_eq!(deserialized.id, log.id);
    }

    #[test]
    fn test_audit_log_serialization_minimal() {
        let log = AuditLog {
            id: Uuid::new_v4(),
            mailbox_id: None,
            action: "system.startup".to_string(),
            resource_type: None,
            resource_id: None,
            details: None,
            ip_address: None,
            user_agent: None,
            created_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&log).unwrap();
        let deserialized: AuditLog = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.action, "system.startup");
        assert!(deserialized.mailbox_id.is_none());
    }

    #[test]
    fn test_build_query_no_filters() {
        let (query, params) = AuditLog::build_query(None, None, 50);
        assert_eq!(query, "SELECT * FROM audit_log WHERE 1=1 ORDER BY created_at DESC LIMIT 50");
        assert!(params.is_empty());
    }

    #[test]
    fn test_build_query_with_mailbox_id() {
        let mid = Uuid::new_v4();
        let (query, params) = AuditLog::build_query(Some(mid), None, 25);
        assert!(query.contains("AND mailbox_id = $1"));
        assert!(query.contains("LIMIT 25"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], mid.to_string());
    }

    #[test]
    fn test_build_query_with_action() {
        let (query, params) = AuditLog::build_query(None, Some("auth.login"), 10);
        assert!(query.contains("AND action = $1"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "auth.login");
    }

    #[test]
    fn test_build_query_with_both_filters() {
        let mid = Uuid::new_v4();
        let (query, params) = AuditLog::build_query(Some(mid), Some("auth.logout"), 100);
        assert!(query.contains("AND mailbox_id = $1"));
        assert!(query.contains("AND action = $2"));
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], mid.to_string());
        assert_eq!(params[1], "auth.logout");
    }

    #[test]
    fn test_audit_log_details_json() {
        let details = serde_json::json!({
            "old_value": "draft",
            "new_value": "sent",
            "message_uid": 42
        });
        let log = AuditLog {
            id: Uuid::new_v4(),
            mailbox_id: None,
            action: "message.status_change".to_string(),
            resource_type: Some("message".to_string()),
            resource_id: Some("42".to_string()),
            details: Some(details.clone()),
            ip_address: None,
            user_agent: None,
            created_at: chrono::Utc::now(),
        };

        assert_eq!(log.details.unwrap()["message_uid"], 42);
    }
}
