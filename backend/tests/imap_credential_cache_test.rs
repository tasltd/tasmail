// Added: the decrypted IMAP credential cache (TMAIL-435) must drop a user's
// entry when their IMAP settings change, or the old password stays in use.

mod common;

use std::time::Instant;

use common::TestApp;
use tasmail::services::imap_service::{invalidate_user_credentials, DecryptedImapCredentials};
use uuid::Uuid;

fn entry(password: &str) -> DecryptedImapCredentials {
    DecryptedImapCredentials {
        host: "imap.example.com".into(),
        port: 993,
        tls: true,
        username: "user@example.com".into(),
        password: password.into(),
        trash_folder: None,
        cached_at: Instant::now(),
    }
}

#[tokio::test]
async fn invalidate_removes_only_that_users_entry() {
    let app = TestApp::new().await;
    let cache = &app.state.imap_credential_cache;
    let (alice, bob) = (Uuid::new_v4(), Uuid::new_v4());
    cache.insert(alice, entry("old-secret"));
    cache.insert(bob, entry("bob-secret"));

    invalidate_user_credentials(&app.state, alice).await;

    assert!(cache.get(&alice).is_none(), "alice's stale password must be gone");
    assert_eq!(cache.get(&bob).map(|c| c.password.clone()).as_deref(), Some("bob-secret"));
}

#[tokio::test]
async fn invalidate_on_empty_cache_is_a_noop() {
    let app = TestApp::new().await;
    invalidate_user_credentials(&app.state, Uuid::new_v4()).await;
    assert!(app.state.imap_credential_cache.is_empty());
}

#[tokio::test]
async fn invalidate_one_of_many_leaves_the_rest() {
    let app = TestApp::new().await;
    let cache = &app.state.imap_credential_cache;
    let ids: Vec<Uuid> = (0..10_000).map(|_| Uuid::new_v4()).collect();
    for id in &ids {
        cache.insert(*id, entry("pw"));
    }

    invalidate_user_credentials(&app.state, ids[4_242]).await;

    assert_eq!(cache.len(), 9_999);
    assert!(cache.get(&ids[4_242]).is_none());
}
