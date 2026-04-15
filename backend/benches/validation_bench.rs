// Added: Criterion benchmarks for input validation functions (TMAIL-38)
// PURPOSE: Measure validation overhead on the request hot path.
// These functions run on every API request, so they must stay fast.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use tasmail::validation;

// Added: Benchmark valid email username validation — the happy path
fn bench_validate_username_valid(c: &mut Criterion) {
    c.bench_function("validate_username_valid", |b| {
        b.iter(|| {
            validation::validate_username("user@example.com").unwrap();
        });
    });
}

// Added: Benchmark username validation with a long but valid email address
fn bench_validate_username_long(c: &mut Criterion) {
    // NOTE: 250 chars is near the RFC 5321 max of 254
    let long_local = "a".repeat(200);
    let long_email = format!("{}@example.com", long_local);

    c.bench_function("validate_username_long_valid", |b| {
        b.iter(|| {
            validation::validate_username(&long_email).unwrap();
        });
    });
}

// Added: Benchmark username rejection (invalid format) — error path overhead
fn bench_validate_username_invalid(c: &mut Criterion) {
    c.bench_function("validate_username_invalid_no_at", |b| {
        b.iter(|| {
            let _ = validation::validate_username("not-an-email");
        });
    });
}

// Added: Benchmark password validation at minimum length boundary
fn bench_validate_password_valid(c: &mut Criterion) {
    c.bench_function("validate_password_valid", |b| {
        b.iter(|| {
            validation::validate_password("strongP@ss1").unwrap();
        });
    });
}

// Added: Benchmark password validation rejection (too short)
fn bench_validate_password_too_short(c: &mut Criterion) {
    c.bench_function("validate_password_too_short", |b| {
        b.iter(|| {
            let _ = validation::validate_password("short");
        });
    });
}

// Added: Benchmark password validation at max boundary (128 chars)
fn bench_validate_password_max_boundary(c: &mut Criterion) {
    let max_pw = "a".repeat(128);

    c.bench_function("validate_password_max_boundary", |b| {
        b.iter(|| {
            validation::validate_password(&max_pw).unwrap();
        });
    });
}

// Added: Benchmark search query validation with various lengths
fn bench_validate_search_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_search_query");

    // NOTE: Test at multiple input sizes to see if validation scales linearly
    for size in [10, 50, 100, 250, 500] {
        let query = "a".repeat(size);
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &query,
            |b, q| {
                b.iter(|| {
                    validation::validate_search_query(q).unwrap();
                });
            },
        );
    }
    group.finish();
}

// Added: Benchmark search query rejection with IMAP injection characters
fn bench_validate_search_query_injection(c: &mut Criterion) {
    c.bench_function("validate_search_query_injection_reject", |b| {
        b.iter(|| {
            let _ = validation::validate_search_query("test\r\nLOGOUT");
        });
    });
}

// Added: Benchmark folder name validation — hit on every IMAP folder operation
fn bench_validate_folder_name(c: &mut Criterion) {
    c.bench_function("validate_folder_name_valid", |b| {
        b.iter(|| {
            validation::validate_folder_name("INBOX").unwrap();
        });
    });
}

// Added: Benchmark subject validation with typical email subject length
fn bench_validate_subject(c: &mut Criterion) {
    c.bench_function("validate_subject_typical", |b| {
        b.iter(|| {
            validation::validate_subject("Re: Meeting tomorrow at 3pm - quarterly review").unwrap();
        });
    });
}

// Added: Benchmark display name validation
fn bench_validate_display_name(c: &mut Criterion) {
    c.bench_function("validate_display_name_valid", |b| {
        b.iter(|| {
            validation::validate_display_name("John Doe").unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_validate_username_valid,
    bench_validate_username_long,
    bench_validate_username_invalid,
    bench_validate_password_valid,
    bench_validate_password_too_short,
    bench_validate_password_max_boundary,
    bench_validate_search_query,
    bench_validate_search_query_injection,
    bench_validate_folder_name,
    bench_validate_subject,
    bench_validate_display_name,
);
criterion_main!(benches);
