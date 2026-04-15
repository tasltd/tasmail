// Added: Shared integration test utilities for building TestApp with full Axum router
// NOTE: Uses tower::ServiceExt oneshot — no real server, no network, no DB/IMAP/SMTP required

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

use tasmail::config::{
    Config, DatabaseConfig, ImapConfig, JwtConfig, ServerConfig, SmtpConfig, StorageConfig,
};
use tasmail::router::create_router;
use tasmail::services::auth_service::Claims;
use tasmail::state::AppState;

// NOTE: JWT secret used across all integration tests — must match test_config()
pub const TEST_JWT_SECRET: &str = "integration-test-secret-key-do-not-use-in-prod";

/// Added: Test application wrapper that holds the Axum router for in-process HTTP testing
pub struct TestApp {
    pub router: Router,
    pub config: Config,
}

impl TestApp {
    /// Added: Build a TestApp with a dummy PgPool (will fail on actual DB queries, but
    /// allows testing routing, middleware, auth, headers, and input validation)
    pub async fn new() -> Self {
        let config = test_config();

        // NOTE: Connect to a non-existent DB — pool creation succeeds but queries will fail.
        // This is intentional: we test HTTP-layer behavior, not DB queries.
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_millis(100))
            .connect_lazy(&config.database.url)
            .unwrap();

        let state = AppState {
            db: pool,
            config: config.clone(),
            // Added: No metrics handle in integration tests (TMAIL-41)
            metrics_handle: None,
        };

        let router = create_router(state);
        TestApp { router, config }
    }

    /// Added: Send a request through the router and return (StatusCode, response body as Value)
    pub async fn request(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        auth_token: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method(method).uri(uri);

        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }

        if let Some(token) = auth_token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token));
        }

        let req_body = match body {
            Some(json) => Body::from(serde_json::to_vec(&json).unwrap()),
            None => Body::empty(),
        };

        let request = builder.body(req_body).unwrap();

        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .unwrap();

        let status = response.status();
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();

        let json: Value = if body_bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body_bytes).unwrap_or(Value::String(
                String::from_utf8_lossy(&body_bytes).to_string(),
            ))
        };

        (status, json)
    }

    /// Added: Send a raw request and return the full axum response (for header inspection)
    pub async fn raw_request(
        &self,
        request: Request<Body>,
    ) -> axum::http::Response<Body> {
        self.router.clone().oneshot(request).await.unwrap()
    }
}

/// Added: Create a test config with dummy values — no real services needed
pub fn test_config() -> Config {
    Config {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        database: DatabaseConfig {
            // NOTE: Points to a non-existent DB intentionally
            url: "postgres://test:test@localhost:59999/nonexistent_test_db".to_string(),
            max_connections: 1,
        },
        imap: ImapConfig {
            host: "127.0.0.1".to_string(),
            port: 993,
            tls: true,
            master_password: None,
        },
        smtp: SmtpConfig {
            host: "127.0.0.1".to_string(),
            port: 587,
            tls: true,
        },
        jwt: JwtConfig {
            secret: TEST_JWT_SECRET.to_string(),
            access_token_expiry_secs: 900,
            refresh_token_expiry_secs: 604800,
        },
        storage: StorageConfig::default(),
        // Added: No metrics token in test config (TMAIL-41)
        metrics_token: None,
        // Added: No Rspamd config in test (TMAIL-15)
        rspamd_url: None,
        rspamd_password: None,
        // Added: No billing config in test (TMAIL-46)
        billing: None,
        // Added: No push notification config in test (TMAIL-50)
        push: None,
    }
}

/// Added: Generate a valid JWT access token for testing protected routes
pub fn create_test_token(user_id: Option<Uuid>, is_admin: bool) -> String {
    let uid = user_id.unwrap_or_else(Uuid::new_v4);
    let now = Utc::now();
    let exp = now + Duration::seconds(900);

    let claims = Claims {
        sub: uid.to_string(),
        username: "testuser@example.com".to_string(),
        is_admin,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .unwrap()
}

/// Added: Generate an expired JWT token for testing expiry handling
pub fn create_expired_token() -> String {
    let past = Utc::now() - Duration::seconds(7200);
    let claims = Claims {
        sub: Uuid::new_v4().to_string(),
        username: "expired@example.com".to_string(),
        is_admin: false,
        exp: (past + Duration::seconds(900)).timestamp() as usize,
        iat: past.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .unwrap()
}
