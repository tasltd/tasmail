// TMAIL-321: Integration test for the scheduled_email_attachments junction
// table. Verifies:
//   * attach_files persists IDs in insertion order
//   * list_attachment_ids returns them in the same order
//   * ON CONFLICT (composite PK) collapses duplicate inserts
//   * Deleting a scheduled_email CASCADEs the junction rows
//   * Deleting an attachment CASCADEs the junction rows
//
// DB-gated: skipped (with a log line) if DATABASE_URL is unreachable, mirroring
// the convention in `rls_context_test.rs` so CI without Postgres stays green.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use tasmail::models::attachment::Attachment;
use tasmail::models::scheduled_email::ScheduledEmail;

fn resolve_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tasmail:tasmail@localhost/tasmail".to_string())
}

async fn try_pool() -> Option<PgPool> {
    let db_url = resolve_db_url();
    match PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect(&db_url)
        .await
    {
        Ok(pool) => {
            // Quick existence check so we don't crash with confusing errors if
            // the migration hasn't run on this DB yet.
            let exists: (bool,) = sqlx::query_as(
                "SELECT EXISTS (
                    SELECT 1 FROM information_schema.tables
                    WHERE table_name = 'scheduled_email_attachments'
                )",
            )
            .fetch_one(&pool)
            .await
            .unwrap_or((false,));
            if !exists.0 {
                eprintln!(
                    "TMAIL-321 scheduled_email_attachments_test: skipping — \
                     scheduled_email_attachments table missing (migration 078 not run)"
                );
                return None;
            }
            Some(pool)
        }
        Err(e) => {
            eprintln!(
                "TMAIL-321 scheduled_email_attachments_test: skipping — \
                 DB unreachable at {}: {}",
                db_url, e
            );
            None
        }
    }
}

/// Seed a domain + mailbox so we can FK-attach a scheduled_email row.
/// Returns `(mailbox_id, domain_id)` — caller is responsible for cleanup.
async fn seed_mailbox(pool: &PgPool) -> Result<(Uuid, Uuid), sqlx::Error> {
    let domain_id = Uuid::new_v4();
    let domain_name = format!("tmail321-{}.example", Uuid::new_v4());
    sqlx::query("INSERT INTO domains (id, name, active) VALUES ($1, $2, true)")
        .bind(domain_id)
        .bind(&domain_name)
        .execute(pool)
        .await?;

    let mailbox_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mailboxes (id, domain_id, username, password_hash, is_admin, active)
         VALUES ($1, $2, $3, 'placeholder-hash', false, true)",
    )
    .bind(mailbox_id)
    .bind(domain_id)
    .bind(format!("tmail321-{}@{}", mailbox_id, domain_name))
    .execute(pool)
    .await?;

    Ok((mailbox_id, domain_id))
}

async fn cleanup_mailbox(pool: &PgPool, mailbox_id: Uuid, domain_id: Uuid) {
    let _ = sqlx::query("DELETE FROM scheduled_emails WHERE mailbox_id = $1")
        .bind(mailbox_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM attachments WHERE mailbox_id = $1")
        .bind(mailbox_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM mailboxes WHERE id = $1")
        .bind(mailbox_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(pool)
        .await;
}

/// Insert two attachments owned by `mailbox_id`. Returns their IDs in insertion
/// order. The on-disk files are NOT created — these tests only exercise the
/// junction table.
async fn seed_two_attachments(
    pool: &PgPool,
    mailbox_id: Uuid,
) -> Result<(Uuid, Uuid), sqlx::Error> {
    let a = Attachment::create(
        pool,
        mailbox_id,
        "a.txt",
        "text/plain",
        7,
        "/tmp/tmail321-a.txt",
        "checksum-a",
    )
    .await?;
    let b = Attachment::create(
        pool,
        mailbox_id,
        "b.txt",
        "text/plain",
        7,
        "/tmp/tmail321-b.txt",
        "checksum-b",
    )
    .await?;
    Ok((a.id, b.id))
}

#[tokio::test]
async fn attach_files_persists_and_lists_in_order() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (mailbox_id, domain_id) = match seed_mailbox(&pool).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("TMAIL-321: failed to seed mailbox: {}", e);
            return;
        }
    };

    // Build a scheduled email row first so the FK from the junction is valid.
    let scheduled = ScheduledEmail::create(
        &pool,
        mailbox_id,
        &["alice@example.com".to_string()],
        &[],
        &[],
        "Test",
        Some("Body"),
        None,
        chrono::Utc::now() + chrono::Duration::seconds(30),
        None,
        &[],
    )
    .await
    .expect("create scheduled email");

    let (att_a, att_b) = seed_two_attachments(&pool, mailbox_id)
        .await
        .expect("seed attachments");

    // Round-trip: attach in [a, b] order, list comes back in [a, b].
    ScheduledEmail::attach_files(&pool, scheduled.id, &[att_a, att_b])
        .await
        .expect("attach_files");
    let listed = ScheduledEmail::list_attachment_ids(&pool, scheduled.id)
        .await
        .expect("list_attachment_ids");
    assert_eq!(listed, vec![att_a, att_b], "order must be preserved");

    // Duplicates collapse — re-attaching the same IDs is a no-op, no error.
    ScheduledEmail::attach_files(&pool, scheduled.id, &[att_a, att_b])
        .await
        .expect("re-attach must not error");
    let listed_after_dup = ScheduledEmail::list_attachment_ids(&pool, scheduled.id)
        .await
        .expect("list after dup");
    assert_eq!(
        listed_after_dup.len(),
        2,
        "ON CONFLICT must dedup; got {:?}",
        listed_after_dup
    );

    cleanup_mailbox(&pool, mailbox_id, domain_id).await;
}

#[tokio::test]
async fn deleting_scheduled_email_cascades_junction_rows() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (mailbox_id, domain_id) = match seed_mailbox(&pool).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("TMAIL-321: failed to seed mailbox: {}", e);
            return;
        }
    };

    let scheduled = ScheduledEmail::create(
        &pool,
        mailbox_id,
        &["alice@example.com".to_string()],
        &[],
        &[],
        "Test",
        Some("Body"),
        None,
        chrono::Utc::now() + chrono::Duration::seconds(30),
        None,
        &[],
    )
    .await
    .expect("create scheduled email");

    let (att_a, _att_b) = seed_two_attachments(&pool, mailbox_id)
        .await
        .expect("seed attachments");
    ScheduledEmail::attach_files(&pool, scheduled.id, &[att_a])
        .await
        .expect("attach_files");

    // Delete the scheduled email row directly. The CASCADE FK should remove
    // the junction row automatically — leaving no orphan link behind.
    sqlx::query("DELETE FROM scheduled_emails WHERE id = $1")
        .bind(scheduled.id)
        .execute(&pool)
        .await
        .expect("delete scheduled email");

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM scheduled_email_attachments WHERE scheduled_email_id = $1",
    )
    .bind(scheduled.id)
    .fetch_one(&pool)
    .await
    .expect("count junction rows");
    assert_eq!(count.0, 0, "junction rows must cascade on scheduled_email delete");

    // The attachment row itself MUST survive — junction CASCADE should NOT
    // touch the parent attachment, only the link.
    let att = Attachment::find_by_id(&pool, att_a)
        .await
        .expect("find attachment");
    assert!(
        att.is_some(),
        "attachment row must survive after scheduled_email delete"
    );

    cleanup_mailbox(&pool, mailbox_id, domain_id).await;
}

#[tokio::test]
async fn deleting_attachment_cascades_junction_rows() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (mailbox_id, domain_id) = match seed_mailbox(&pool).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("TMAIL-321: failed to seed mailbox: {}", e);
            return;
        }
    };

    let scheduled = ScheduledEmail::create(
        &pool,
        mailbox_id,
        &["alice@example.com".to_string()],
        &[],
        &[],
        "Test",
        Some("Body"),
        None,
        chrono::Utc::now() + chrono::Duration::seconds(30),
        None,
        &[],
    )
    .await
    .expect("create scheduled email");

    let (att_a, att_b) = seed_two_attachments(&pool, mailbox_id)
        .await
        .expect("seed attachments");
    ScheduledEmail::attach_files(&pool, scheduled.id, &[att_a, att_b])
        .await
        .expect("attach_files");

    // Delete one attachment — its junction row should disappear, the other
    // should be untouched.
    Attachment::delete(&pool, att_a, mailbox_id)
        .await
        .expect("delete attachment");

    let remaining = ScheduledEmail::list_attachment_ids(&pool, scheduled.id)
        .await
        .expect("list after attachment delete");
    assert_eq!(
        remaining,
        vec![att_b],
        "deleted attachment must drop from junction; got {:?}",
        remaining
    );

    cleanup_mailbox(&pool, mailbox_id, domain_id).await;
}
