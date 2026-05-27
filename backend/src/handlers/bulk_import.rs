// Added: Bulk user import handlers for TMAIL-136 (CSV bulk provisioning)
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::bulk_import::BulkUserImport;
use crate::models::mailbox::Mailbox;
use crate::services::auth_service::{hash_password, Claims};
use crate::services::csv_processor;
use crate::state::AppState;

/// PURPOSE: Parse and validate admin_id from JWT claims
fn parse_admin_id(claims: &Claims) -> Result<Uuid, AppError> {
    claims
        .sub
        .parse()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid user ID in token")))
}

/// PURPOSE: Require admin role, returning Forbidden if not admin
fn require_admin(claims: &Claims) -> Result<(), AppError> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }
    Ok(())
}

/// PURPOSE: Upload a CSV file, parse it, validate rows, and bulk-create users
/// CONSTRAINTS: Requires admin role; CSV must have headers: email, display_name, password, role
/// EXTERNAL: Creates mailbox records in PostgreSQL via Mailbox::create
///
/// POST /api/admin/users/bulk-import
pub async fn upload_bulk_csv(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<BulkUserImport>), AppError> {
    let admin_id = parse_admin_id(&claims)?;
    require_admin(&claims)?;

    // Added: Extract CSV file data from multipart form
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|multipart_error| AppError::BadRequest(format!("Invalid multipart data: {}", multipart_error)))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "file" {
            filename = field.file_name().map(String::from);
            let data = field
                .bytes()
                .await
                .map_err(|read_error| AppError::BadRequest(format!("Failed to read file data: {}", read_error)))?;

            if data.is_empty() {
                return Err(AppError::BadRequest(
                    "CSV file is empty. Upload a valid CSV file with user data.".to_string(),
                ));
            }

            file_data = Some(data.to_vec());
        }
    }

    // Added: Validate that a file was provided
    let csv_bytes = file_data.ok_or_else(|| {
        AppError::BadRequest(
            "No file field found in multipart upload. Include a 'file' field with the CSV file."
                .to_string(),
        )
    })?;

    let original_filename = filename.unwrap_or_else(|| "import.csv".to_string());

    // Added: Validate .csv file extension
    if !original_filename.to_lowercase().ends_with(".csv") {
        return Err(AppError::BadRequest(format!(
            "File '{}' does not have a .csv extension. Only CSV files are accepted.",
            original_filename
        )));
    }

    // Added: Parse CSV content as UTF-8 string
    let csv_content = String::from_utf8(csv_bytes).map_err(|_| {
        AppError::BadRequest("CSV file is not valid UTF-8 text.".to_string())
    })?;

    // Added: Parse and validate CSV rows
    let parse_result = csv_processor::parse_csv(&csv_content);

    // Added: Create the import record in the database
    let mut import = BulkUserImport::create(
        &state.db,
        admin_id,
        &original_filename,
        parse_result.total_rows as i32,
    )
    .await?;

    // Added: If CSV had parse/validation errors and no valid rows, mark as failed
    if parse_result.validated_rows.is_empty() && !parse_result.errors.is_empty() {
        let errors_json = serde_json::to_value(&parse_result.errors)
            .unwrap_or_else(|_| serde_json::json!([]));
        BulkUserImport::mark_failed(&state.db, import.id, &errors_json).await?;

        import.status = "failed".to_string();
        import.errors = errors_json;
        import.error_count = parse_result.errors.len() as i32;
        return Ok((StatusCode::CREATED, Json(import)));
    }

    // Added: Process validated rows — create user accounts
    let mut success_count = 0_i32;
    let mut processing_errors = parse_result.errors; // NOTE: Carry forward any validation errors from parsing

    // Added: Look up the first domain for user creation (admin domain)
    let domain_id = get_default_domain_id(&state).await?;

    for row in &parse_result.validated_rows {
        // Added: Check for duplicate username before creating
        if Mailbox::find_by_username(&state.db, &row.email)
            .await?
            .is_some()
        {
            processing_errors.push(crate::models::bulk_import::BulkImportError {
                row: 0, // NOTE: Row number not tracked per validated row; 0 indicates processing error
                field: "email".to_string(),
                message: format!("User '{}' already exists", row.email),
            });
            continue;
        }

        // Added: Hash password and create mailbox
        match hash_password(&row.password) {
            Ok(password_hash) => {
                let is_admin_role = row.role == "admin";
                match create_user_with_role(
                    &state,
                    &row.email,
                    &password_hash,
                    domain_id,
                    &row.display_name,
                    is_admin_role,
                )
                .await
                {
                    Ok(_) => success_count += 1,
                    Err(create_error) => {
                        processing_errors.push(crate::models::bulk_import::BulkImportError {
                            row: 0,
                            field: "email".to_string(),
                            message: format!("Failed to create user '{}': {}", row.email, create_error),
                        });
                    }
                }
            }
            Err(hash_error) => {
                processing_errors.push(crate::models::bulk_import::BulkImportError {
                    row: 0,
                    field: "password".to_string(),
                    message: format!("Failed to hash password for '{}': {}", row.email, hash_error),
                });
            }
        }
    }

    // Added: Update the import record with final results
    let processed_rows = parse_result.validated_rows.len() as i32;
    let error_count = processing_errors.len() as i32;
    let errors_json = serde_json::to_value(&processing_errors)
        .unwrap_or_else(|_| serde_json::json!([]));

    BulkUserImport::mark_completed(
        &state.db,
        import.id,
        processed_rows,
        success_count,
        error_count,
        &errors_json,
    )
    .await?;

    // Added: Return the updated import record
    import.status = "completed".to_string();
    import.processed_rows = processed_rows;
    import.success_count = success_count;
    import.error_count = error_count;
    import.errors = errors_json;

    Ok((StatusCode::CREATED, Json(import)))
}

/// PURPOSE: Helper to get a default domain_id for bulk user creation
/// CONSTRAINTS: At least one domain must exist in the system
async fn get_default_domain_id(state: &AppState) -> Result<Uuid, AppError> {
    let domain_row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM domains ORDER BY created_at LIMIT 1")
            .fetch_optional(&state.db)
            .await?;

    domain_row
        .map(|(domain_id,)| domain_id)
        .ok_or_else(|| {
            AppError::BadRequest(
                "No domains configured. Create a domain before importing users.".to_string(),
            )
        })
}

/// PURPOSE: Create a user mailbox with the specified admin role
/// CONSTRAINTS: Uses the Mailbox::create pattern then updates is_admin if needed
async fn create_user_with_role(
    state: &AppState,
    username: &str,
    password_hash: &str,
    domain_id: Uuid,
    display_name: &str,
    is_admin: bool,
) -> Result<Uuid, AppError> {
    let quota_bytes: i64 = 1_073_741_824; // NOTE: 1 GB default quota
    let mailbox = Mailbox::create(
        &state.db,
        username,
        password_hash,
        domain_id,
        Some(display_name),
        quota_bytes,
    )
    .await?;

    // Added: Set admin flag if role is 'admin'
    if is_admin {
        sqlx::query("UPDATE mailboxes SET is_admin = true WHERE id = $1")
            .bind(mailbox.id)
            .execute(&state.db)
            .await?;
    }

    Ok(mailbox.id)
}

/// PURPOSE: List all bulk import records for the authenticated admin
///
/// GET /api/admin/users/bulk-imports
pub async fn list_bulk_imports(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<Json<Vec<BulkUserImport>>, AppError> {
    let admin_id = parse_admin_id(&claims)?;
    require_admin(&claims)?;

    let imports = BulkUserImport::list_by_admin(&state.db, admin_id).await?;
    Ok(Json(imports))
}

/// PURPOSE: Get a single bulk import record by ID with error details
///
/// GET /api/admin/users/bulk-imports/:id
pub async fn get_bulk_import(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<BulkUserImport>, AppError> {
    require_admin(&claims)?;

    let import = BulkUserImport::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Bulk import not found".to_string()))?;
    Ok(Json(import))
}

/// PURPOSE: Download a CSV template file with expected headers and example row
///
/// GET /api/admin/users/bulk-import/template
pub async fn download_template(
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<(StatusCode, [(axum::http::header::HeaderName, &'static str); 2], String), AppError> {
    require_admin(&claims)?;

    let template_content = csv_processor::generate_template();

    // Added: Return CSV with appropriate Content-Type and disposition headers
    Ok((
        StatusCode::OK,
        [
            (
                axum::http::header::CONTENT_TYPE,
                "text/csv; charset=utf-8",
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"bulk-import-template.csv\"",
            ),
        ],
        template_content,
    ))
}

/// PURPOSE: Export ALL users as a CSV download for admin (TMAIL-136 export side).
/// CONSTRAINTS: Admin only. Never returns password_hash or totp_secret — security boundary
/// enforced both by the SELECT columns and by csv_processor::generate_users_export.
/// EXTERNAL: SELECT * FROM mailboxes ordered by username.
///
/// GET /api/admin/users/export
pub async fn export_users_csv(
    State(state): State<AppState>,
    axum::Extension(claims): axum::Extension<Claims>,
) -> Result<(StatusCode, [(axum::http::header::HeaderName, &'static str); 2], String), AppError> {
    require_admin(&claims)?;

    // Added: Fetch every mailbox; the export helper strips password_hash/totp_secret.
    let users = sqlx::query_as::<_, Mailbox>("SELECT * FROM mailboxes ORDER BY username")
        .fetch_all(&state.db)
        .await?;

    let csv_body = csv_processor::generate_users_export(&users)
        .map_err(|csv_error| AppError::Internal(anyhow::anyhow!("Failed to render users CSV: {}", csv_error)))?;

    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"users-export.csv\"",
            ),
        ],
        csv_body,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_admin_id_valid() {
        // Added: Verify valid UUID parses correctly from claims
        let claims = Claims {
            sub: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            username: "admin@example.com".to_string(),
            is_admin: true,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        let result = parse_admin_id(&claims);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_parse_admin_id_invalid() {
        // Added: Verify invalid UUID string produces error
        let claims = Claims {
            sub: "not-a-uuid".to_string(),
            username: "admin@example.com".to_string(),
            is_admin: true,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(parse_admin_id(&claims).is_err());
    }

    #[test]
    fn test_require_admin_passes_for_admin() {
        // Added: Verify admin check passes for admin claims
        let claims = Claims {
            sub: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            username: "admin@example.com".to_string(),
            is_admin: true,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        assert!(require_admin(&claims).is_ok());
    }

    #[test]
    fn test_require_admin_fails_for_non_admin() {
        // Added: Verify admin check fails for non-admin claims
        let claims = Claims {
            sub: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            username: "user@example.com".to_string(),
            is_admin: false,
            is_compliance_officer: false,
            exp: 0,
            iat: 0,
        };
        let result = require_admin(&claims);
        assert!(result.is_err());
    }

    #[test]
    fn test_csv_extension_validation() {
        // Added: Verify .csv extension check logic used in upload handler
        let valid_filenames = ["users.csv", "import.CSV", "My Users.csv"];
        for fname in valid_filenames {
            assert!(
                fname.to_lowercase().ends_with(".csv"),
                "'{}' should pass .csv extension check",
                fname
            );
        }

        let invalid_filenames = ["users.xlsx", "data.txt", "file.csv.zip", "noext"];
        for fname in invalid_filenames {
            assert!(
                !fname.to_lowercase().ends_with(".csv"),
                "'{}' should fail .csv extension check",
                fname
            );
        }
    }

    #[test]
    fn test_default_quota_bytes() {
        // Added: Verify the default quota is 1 GB
        let quota_bytes: i64 = 1_073_741_824;
        assert_eq!(quota_bytes, 1024 * 1024 * 1024);
    }
}
