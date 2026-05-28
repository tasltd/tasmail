// Added: Criterion benchmarks for authentication critical paths (TMAIL-38)
// PURPOSE: Measure Argon2id hashing, JWT creation, and JWT validation performance
// to establish baselines and detect regressions in the auth hot path.

use criterion::{criterion_group, criterion_main, Criterion};

use chrono::Utc;
use uuid::Uuid;

use tasmail::config::JwtConfig;
use tasmail::models::mailbox::Mailbox;
use tasmail::services::auth_service;

// Added: Reusable test JWT config matching the unit test pattern in auth_service.rs
fn bench_jwt_config() -> JwtConfig {
    JwtConfig {
        secret: "bench-secret-key-for-criterion-tests".to_string(),
        access_token_expiry_secs: 900,
        refresh_token_expiry_secs: 604800,
    }
}

// Added: Reusable test mailbox for JWT benchmarks
fn bench_mailbox() -> Mailbox {
    Mailbox {
        id: Uuid::new_v4(),
        domain_id: Uuid::new_v4(),
        username: "bench@example.com".to_string(),
        password_hash: String::new(), // NOTE: Not used in JWT benchmarks
        display_name: Some("Bench User".to_string()),
        quota_bytes: 1_073_741_824,
        quota_warn_percent: 80,
        active: true,
        is_admin: false,
        is_compliance_officer: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        totp_secret: None,
        totp_enabled: false,
        totp_verified_at: None,
        failed_login_attempts: 0,
        last_failed_login_at: None,
        locked_until: None,
    }
}

// Added: Benchmark Argon2id password hashing — the most CPU-expensive auth operation
fn bench_argon2_hash(c: &mut Criterion) {
    c.bench_function("argon2id_hash_password", |b| {
        b.iter(|| {
            auth_service::hash_password("benchmark_password_123!").unwrap();
        });
    });
}

// Added: Benchmark Argon2id password verification against a pre-computed hash
fn bench_argon2_verify(c: &mut Criterion) {
    // NOTE: Pre-compute the hash outside the benchmark loop to isolate verification cost
    let hash = auth_service::hash_password("benchmark_password_123!").unwrap();

    c.bench_function("argon2id_verify_password", |b| {
        b.iter(|| {
            auth_service::verify_password("benchmark_password_123!", &hash).unwrap();
        });
    });
}

// Added: Benchmark JWT access token creation (HMAC-SHA256 signing)
fn bench_jwt_create(c: &mut Criterion) {
    let config = bench_jwt_config();
    let mailbox = bench_mailbox();

    c.bench_function("jwt_create_access_token", |b| {
        b.iter(|| {
            auth_service::create_access_token(&config, &mailbox).unwrap();
        });
    });
}

// Added: Benchmark JWT access token validation (HMAC-SHA256 verification + claims decode)
fn bench_jwt_validate(c: &mut Criterion) {
    let config = bench_jwt_config();
    let mailbox = bench_mailbox();
    // NOTE: Pre-create a valid token outside the loop to isolate validation cost
    let token = auth_service::create_access_token(&config, &mailbox).unwrap();

    c.bench_function("jwt_validate_access_token", |b| {
        b.iter(|| {
            auth_service::validate_access_token(&config, &token).unwrap();
        });
    });
}

// Added: Benchmark refresh token generation (UUID v4)
fn bench_refresh_token_gen(c: &mut Criterion) {
    c.bench_function("generate_refresh_token", |b| {
        b.iter(|| {
            auth_service::generate_refresh_token();
        });
    });
}

// Added: Benchmark refresh token hashing (SHA-256)
fn bench_refresh_token_hash(c: &mut Criterion) {
    let token = auth_service::generate_refresh_token();

    c.bench_function("hash_refresh_token_sha256", |b| {
        b.iter(|| {
            auth_service::hash_refresh_token(&token);
        });
    });
}

criterion_group!(
    benches,
    bench_argon2_hash,
    bench_argon2_verify,
    bench_jwt_create,
    bench_jwt_validate,
    bench_refresh_token_gen,
    bench_refresh_token_hash,
);
criterion_main!(benches);
