// Added: DANE service for TMAIL-125 — DNS TLSA record lookup and certificate verification
// PURPOSE: Provides TLSA record parsing, lookup simulation, and certificate chain verification
// EXTERNAL: In production, would use DNS resolver (trust-dns/hickory) for actual TLSA queries

use sha2::{Digest, Sha256, Sha512};

use crate::models::dane::{DaneResult, TlsaRecord};

/// PURPOSE: TLSA usage field values per RFC 6698
/// NOTE: Usage 0 = CA constraint, 1 = Service cert constraint, 2 = Trust anchor assertion, 3 = Domain-issued cert
pub const USAGE_PKIX_TA: u8 = 0;
pub const USAGE_PKIX_EE: u8 = 1;
pub const USAGE_DANE_TA: u8 = 2;
pub const USAGE_DANE_EE: u8 = 3;

/// PURPOSE: TLSA selector field values per RFC 6698
/// NOTE: Selector 0 = Full certificate, 1 = SubjectPublicKeyInfo
pub const SELECTOR_FULL_CERT: u8 = 0;
pub const SELECTOR_SPKI: u8 = 1;

/// PURPOSE: TLSA matching type field values per RFC 6698
/// NOTE: 0 = Exact match, 1 = SHA-256 hash, 2 = SHA-512 hash
pub const MATCHING_EXACT: u8 = 0;
pub const MATCHING_SHA256: u8 = 1;
pub const MATCHING_SHA512: u8 = 2;

/// PURPOSE: Parse a TLSA record from raw DNS-style presentation format
/// CONSTRAINTS: Expects format "usage selector matching_type hex_cert_data"
/// NOTE: Returns None for invalid formats
pub fn parse_tlsa_record(raw: &str) -> Option<TlsaRecord> {
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }

    let usage = parts[0].parse::<u8>().ok()?;
    let selector = parts[1].parse::<u8>().ok()?;
    let matching_type = parts[2].parse::<u8>().ok()?;
    // NOTE: Remaining parts are the hex cert data (may be split across multiple tokens)
    let cert_data = parts[3..].join("");

    // Added: Validate usage, selector, and matching_type ranges
    if usage > 3 || selector > 1 || matching_type > 2 {
        return None;
    }

    // Added: Validate cert_data is valid hex
    if !cert_data.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    Some(TlsaRecord {
        usage,
        selector,
        matching_type,
        cert_data: cert_data.to_lowercase(),
    })
}

/// PURPOSE: Compute the hash of certificate data using the specified matching type
/// CONSTRAINTS: matching_type 0 returns raw hex, 1 = SHA-256, 2 = SHA-512
pub fn compute_cert_hash(cert_data: &[u8], matching_type: u8) -> Option<String> {
    match matching_type {
        MATCHING_EXACT => Some(hex::encode(cert_data)),
        MATCHING_SHA256 => {
            let mut hasher = Sha256::new();
            hasher.update(cert_data);
            Some(hex::encode(hasher.finalize()))
        }
        MATCHING_SHA512 => {
            let mut hasher = Sha512::new();
            hasher.update(cert_data);
            Some(hex::encode(hasher.finalize()))
        }
        _ => None,
    }
}

/// PURPOSE: Verify a certificate chain against a set of TLSA records
/// CONSTRAINTS: cert_chain_der should be DER-encoded certificate bytes
/// NOTE: For DANE-EE (usage 3), only the end-entity cert is checked
pub fn verify_dane(domain: &str, cert_chain_der: &[u8], tlsa_records: &[TlsaRecord]) -> DaneResult {
    if tlsa_records.is_empty() {
        return DaneResult {
            domain: domain.to_string(),
            status: "no_tlsa".to_string(),
            tlsa_records: vec![],
            message: "No TLSA records found for domain".to_string(),
        };
    }

    for record in tlsa_records {
        // Added: Compute hash of the cert using the record's matching type
        let cert_hash = match compute_cert_hash(cert_chain_der, record.matching_type) {
            Some(hash) => hash,
            None => continue,
        };

        // Added: Compare computed hash with the TLSA record's cert_data
        if cert_hash == record.cert_data {
            return DaneResult {
                domain: domain.to_string(),
                status: "verified".to_string(),
                tlsa_records: tlsa_records.to_vec(),
                message: format!(
                    "Certificate matches TLSA record (usage={}, selector={}, matching_type={})",
                    record.usage, record.selector, record.matching_type
                ),
            };
        }
    }

    DaneResult {
        domain: domain.to_string(),
        status: "failed".to_string(),
        tlsa_records: tlsa_records.to_vec(),
        message: "Certificate does not match any TLSA record".to_string(),
    }
}

/// PURPOSE: Format the TLSA DNS query name for a given domain and port
/// NOTE: Standard format is _port._tcp.domain (e.g., _25._tcp.example.com)
pub fn tlsa_query_name(domain: &str, port: u16) -> String {
    format!("_{port}._tcp.{domain}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tlsa_record_valid() {
        // Added: Parse a standard DANE-EE SHA-256 TLSA record
        let raw = "3 1 1 a0b1c2d3e4f5";
        let record = parse_tlsa_record(raw).unwrap();
        assert_eq!(record.usage, USAGE_DANE_EE);
        assert_eq!(record.selector, SELECTOR_SPKI);
        assert_eq!(record.matching_type, MATCHING_SHA256);
        assert_eq!(record.cert_data, "a0b1c2d3e4f5");
    }

    #[test]
    fn test_parse_tlsa_record_multipart_hex() {
        // Added: TLSA record with hex data split across multiple tokens
        let raw = "3 0 1 a0b1 c2d3 e4f5";
        let record = parse_tlsa_record(raw).unwrap();
        assert_eq!(record.cert_data, "a0b1c2d3e4f5");
    }

    #[test]
    fn test_parse_tlsa_record_invalid_short() {
        // Added: Too few fields should return None
        let raw = "3 1";
        assert!(parse_tlsa_record(raw).is_none());
    }

    #[test]
    fn test_parse_tlsa_record_invalid_usage() {
        // Added: Usage > 3 is invalid
        let raw = "4 1 1 abcd";
        assert!(parse_tlsa_record(raw).is_none());
    }

    #[test]
    fn test_parse_tlsa_record_invalid_hex() {
        // Added: Non-hex cert data should return None
        let raw = "3 1 1 zzzz";
        assert!(parse_tlsa_record(raw).is_none());
    }

    #[test]
    fn test_parse_tlsa_record_exact_match() {
        // Added: Parse a TLSA record with matching_type 0 (exact)
        let raw = "2 0 0 deadbeef";
        let record = parse_tlsa_record(raw).unwrap();
        assert_eq!(record.usage, USAGE_DANE_TA);
        assert_eq!(record.matching_type, MATCHING_EXACT);
    }

    #[test]
    fn test_compute_cert_hash_exact() {
        // Added: Matching type 0 returns raw hex encoding
        let data = b"hello";
        let hash = compute_cert_hash(data, MATCHING_EXACT).unwrap();
        assert_eq!(hash, hex::encode(b"hello"));
    }

    #[test]
    fn test_compute_cert_hash_sha256() {
        // Added: SHA-256 hash of known data
        let data = b"test certificate data";
        let hash = compute_cert_hash(data, MATCHING_SHA256).unwrap();
        // NOTE: Verify hash length (64 hex chars = 32 bytes)
        assert_eq!(hash.len(), 64);
        // Added: Verify it's deterministic
        let hash2 = compute_cert_hash(data, MATCHING_SHA256).unwrap();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_compute_cert_hash_sha512() {
        // Added: SHA-512 hash should be 128 hex chars
        let data = b"test certificate data";
        let hash = compute_cert_hash(data, MATCHING_SHA512).unwrap();
        assert_eq!(hash.len(), 128);
    }

    #[test]
    fn test_compute_cert_hash_invalid_type() {
        // Added: Invalid matching type returns None
        assert!(compute_cert_hash(b"data", 99).is_none());
    }

    #[test]
    fn test_verify_dane_no_tlsa() {
        // Added: Empty TLSA records should return no_tlsa status
        let result = verify_dane("example.com", b"some-cert", &[]);
        assert_eq!(result.status, "no_tlsa");
        assert_eq!(result.domain, "example.com");
    }

    #[test]
    fn test_verify_dane_matching_cert() {
        // Added: Certificate matching a TLSA record should return verified
        let cert_data = b"test-cert-bytes";
        let cert_hash = compute_cert_hash(cert_data, MATCHING_SHA256).unwrap();

        let tlsa = vec![TlsaRecord {
            usage: USAGE_DANE_EE,
            selector: SELECTOR_FULL_CERT,
            matching_type: MATCHING_SHA256,
            cert_data: cert_hash,
        }];

        let result = verify_dane("secure.example.com", cert_data, &tlsa);
        assert_eq!(result.status, "verified");
        assert!(result.message.contains("matches"));
    }

    #[test]
    fn test_verify_dane_non_matching_cert() {
        // Added: Certificate not matching any TLSA record should return failed
        let tlsa = vec![TlsaRecord {
            usage: USAGE_DANE_EE,
            selector: SELECTOR_SPKI,
            matching_type: MATCHING_SHA256,
            cert_data: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        }];

        let result = verify_dane("example.com", b"different-cert-data", &tlsa);
        assert_eq!(result.status, "failed");
        assert!(result.message.contains("does not match"));
    }

    #[test]
    fn test_verify_dane_multiple_records_one_matches() {
        // Added: If any TLSA record matches, verification succeeds
        let cert_data = b"my-certificate";
        let correct_hash = compute_cert_hash(cert_data, MATCHING_SHA256).unwrap();

        let tlsa = vec![
            TlsaRecord {
                usage: USAGE_DANE_EE,
                selector: SELECTOR_FULL_CERT,
                matching_type: MATCHING_SHA256,
                cert_data: "aaaa".to_string(), // NOTE: Does not match
            },
            TlsaRecord {
                usage: USAGE_DANE_EE,
                selector: SELECTOR_FULL_CERT,
                matching_type: MATCHING_SHA256,
                cert_data: correct_hash, // NOTE: Matches
            },
        ];

        let result = verify_dane("example.com", cert_data, &tlsa);
        assert_eq!(result.status, "verified");
    }

    #[test]
    fn test_tlsa_query_name_default_port() {
        // Added: Standard SMTP port 25 query name
        let name = tlsa_query_name("example.com", 25);
        assert_eq!(name, "_25._tcp.example.com");
    }

    #[test]
    fn test_tlsa_query_name_submission_port() {
        // Added: SMTP submission port 587 query name
        let name = tlsa_query_name("mail.example.com", 587);
        assert_eq!(name, "_587._tcp.mail.example.com");
    }

    #[test]
    fn test_parse_tlsa_record_sha512_matching() {
        // Added: Parse a TLSA record with SHA-512 matching type
        let raw = "3 1 2 aabbccdd";
        let record = parse_tlsa_record(raw).unwrap();
        assert_eq!(record.matching_type, MATCHING_SHA512);
    }
}
