use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::migration_job::{
    CreateImapMigrationRequest, CreateMboxImportRequest, MigrationJob,
};
use crate::services::auth_service::Claims;
use crate::state::AppState;

fn parse_mailbox_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid mailbox ID")))
}

/// POST /api/migration/imap — Start an IMAP-to-IMAP migration
pub async fn start_imap_migration(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateImapMigrationRequest>,
) -> Result<(StatusCode, Json<MigrationJob>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    // Validate required fields
    if body.source_host.is_empty() || body.source_user.is_empty() || body.source_password.is_empty() {
        return Err(AppError::BadRequest("Source host, user, and password are required".to_string()));
    }

    let job = MigrationJob::create_imap(&state.db, mailbox_id, &body).await?;

    // NOTE: The actual migration execution is handled by a background worker
    // that polls for pending jobs. In production, this would invoke imapsync.
    // For now, just create the job record.

    Ok((StatusCode::CREATED, Json(job)))
}

/// POST /api/migration/mbox — Start an MBOX file import
pub async fn start_mbox_import(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Json(body): Json<CreateMboxImportRequest>,
) -> Result<(StatusCode, Json<MigrationJob>), AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    if body.mbox_file_path.is_empty() {
        return Err(AppError::BadRequest("MBOX file path is required".to_string()));
    }

    let job = MigrationJob::create_mbox(&state.db, mailbox_id, &body).await?;

    Ok((StatusCode::CREATED, Json(job)))
}

/// GET /api/migration — List migration jobs for the current user
pub async fn list_migrations(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<MigrationJob>>, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;
    let jobs = MigrationJob::list_by_mailbox(&state.db, mailbox_id).await?;
    Ok(Json(jobs))
}

/// GET /api/migration/:id — Get migration job status
pub async fn get_migration(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MigrationJob>, AppError> {
    let job = MigrationJob::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Migration job not found".to_string()))?;
    Ok(Json(job))
}

/// POST /api/migration/:id/cancel — Cancel a pending/running migration
pub async fn cancel_migration(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = parse_mailbox_id(&claims)?;

    let job = MigrationJob::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Migration job not found".to_string()))?;

    if job.mailbox_id != mailbox_id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the job owner".to_string()));
    }

    if job.status != "pending" && job.status != "running" {
        return Err(AppError::BadRequest(
            format!("Cannot cancel job in '{}' state", job.status),
        ));
    }

    MigrationJob::cancel(&state.db, id).await?;
    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imap_migration_request_deserialization() {
        let json = r#"{
            "source_host": "imap.gmail.com",
            "source_port": 993,
            "source_user": "user@gmail.com",
            "source_password": "app-password",
            "source_use_ssl": true
        }"#;
        let req: CreateImapMigrationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.source_host, "imap.gmail.com");
        assert_eq!(req.source_port, Some(993));
        assert_eq!(req.source_user, "user@gmail.com");
        assert!(req.source_use_ssl.unwrap_or(false));
    }

    #[test]
    fn test_imap_migration_request_minimal() {
        let json = r#"{
            "source_host": "imap.example.com",
            "source_user": "user",
            "source_password": "pass"
        }"#;
        let req: CreateImapMigrationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.source_host, "imap.example.com");
        assert!(req.source_port.is_none());
        assert!(req.source_use_ssl.is_none());
    }

    #[test]
    fn test_mbox_import_request_deserialization() {
        let json = r#"{"mbox_file_path": "/tmp/mail.mbox"}"#;
        let req: CreateMboxImportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mbox_file_path, "/tmp/mail.mbox");
    }

    #[test]
    fn test_mbox_import_request_rejects_missing_path() {
        let json = r#"{}"#;
        assert!(serde_json::from_str::<CreateMboxImportRequest>(json).is_err());
    }

    #[test]
    fn test_parse_mailbox_id_valid() {
        let claims = Claims {
            sub: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            username: "test@example.com".to_string(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_ok());
    }

    #[test]
    fn test_parse_mailbox_id_invalid() {
        let claims = Claims {
            sub: "invalid".to_string(),
            username: "test@example.com".to_string(),
            is_admin: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_mailbox_id(&claims).is_err());
    }
}
