use axum::{extract::Path, http::StatusCode, Json, Extension};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::Mailbox;
use crate::models::migration_job::{
    CreateImapMigrationRequest, CreateMboxImportRequest, MigrationJob,
};
use crate::services::auth_service::Claims;
use crate::services::db_session::RlsConn;

/// POST /api/migration/imap — Start an IMAP-to-IMAP migration
pub async fn start_imap_migration(
    mut rls: RlsConn,
    mailbox: Mailbox,
    Json(body): Json<CreateImapMigrationRequest>,
) -> Result<(StatusCode, Json<MigrationJob>), AppError> {
    let mailbox_id = mailbox.0.id;

    if body.source_host.is_empty()
        || body.source_user.is_empty()
        || body.source_password.is_empty()
    {
        return Err(AppError::BadRequest(
            "Source host, user, and password are required".to_string(),
        ));
    }

    let job = MigrationJob::create_imap_conn(&mut *rls, mailbox_id, &body).await?;

    Ok((StatusCode::CREATED, Json(job)))
}

/// POST /api/migration/mbox — Start an MBOX file import
pub async fn start_mbox_import(
    mut rls: RlsConn,
    mailbox: Mailbox,
    Json(body): Json<CreateMboxImportRequest>,
) -> Result<(StatusCode, Json<MigrationJob>), AppError> {
    let mailbox_id = mailbox.0.id;

    if body.mbox_file_path.is_empty() {
        return Err(AppError::BadRequest("MBOX file path is required".to_string()));
    }

    let job = MigrationJob::create_mbox_conn(&mut *rls, mailbox_id, &body).await?;

    Ok((StatusCode::CREATED, Json(job)))
}

/// GET /api/migration — List migration jobs for the current user
pub async fn list_migrations(
    mut rls: RlsConn,
    mailbox: Mailbox,
) -> Result<Json<Vec<MigrationJob>>, AppError> {
    let mailbox_id = mailbox.0.id;
    let jobs = MigrationJob::list_by_mailbox_conn(&mut *rls, mailbox_id).await?;
    Ok(Json(jobs))
}

/// GET /api/migration/:id — Get migration job status
pub async fn get_migration(
    mut rls: RlsConn,
    Path(id): Path<Uuid>,
) -> Result<Json<MigrationJob>, AppError> {
    let job = MigrationJob::find_by_id_conn(&mut *rls, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Migration job not found".to_string()))?;
    Ok(Json(job))
}

/// POST /api/migration/:id/cancel — Cancel a pending/running migration
pub async fn cancel_migration(
    mut rls: RlsConn,
    mailbox: Mailbox,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mailbox_id = mailbox.0.id;

    let job = MigrationJob::find_by_id_conn(&mut *rls, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Migration job not found".to_string()))?;

    if job.mailbox_id != mailbox_id && !claims.is_admin {
        return Err(AppError::Forbidden("Not the job owner".to_string()));
    }

    if job.status != "pending" && job.status != "running" {
        return Err(AppError::BadRequest(format!(
            "Cannot cancel job in '{}' state",
            job.status
        )));
    }

    MigrationJob::cancel_conn(&mut *rls, id).await?;
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
}
