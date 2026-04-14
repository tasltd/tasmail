// Added: CSV parsing and validation service for bulk user import (TMAIL-136)
use crate::models::bulk_import::{BulkImportError, BulkImportRow};

/// PURPOSE: Result of parsing and validating a CSV file
/// CONSTRAINTS: validated_rows only contains rows that passed all validation checks
pub struct CsvParseResult {
    pub validated_rows: Vec<BulkImportRow>,
    pub errors: Vec<BulkImportError>,
    pub total_rows: usize,
}

/// PURPOSE: Parse CSV content with headers: email, display_name, password, role
/// CONSTRAINTS: Expects UTF-8 encoded CSV with comma delimiter and header row
/// EXTERNAL: Uses the csv crate for RFC 4180-compliant parsing
pub fn parse_csv(content: &str) -> CsvParseResult {
    let mut validated_rows = Vec::new();
    let mut errors = Vec::new();

    // Added: Handle empty input gracefully
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return CsvParseResult {
            validated_rows,
            errors: vec![BulkImportError {
                row: 0,
                field: "file".to_string(),
                message: "CSV file is empty".to_string(),
            }],
            total_rows: 0,
        };
    }

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(trimmed.as_bytes());

    // Added: Validate expected headers are present
    let headers = match reader.headers() {
        Ok(headers) => headers.clone(),
        Err(csv_error) => {
            return CsvParseResult {
                validated_rows,
                errors: vec![BulkImportError {
                    row: 0,
                    field: "headers".to_string(),
                    message: format!("Failed to parse CSV headers: {}", csv_error),
                }],
                total_rows: 0,
            };
        }
    };

    let expected_headers = ["email", "display_name", "password", "role"];
    let header_names: Vec<&str> = headers.iter().collect();
    for expected_header in &expected_headers {
        if !header_names.contains(expected_header) {
            errors.push(BulkImportError {
                row: 0,
                field: "headers".to_string(),
                message: format!(
                    "Missing required header '{}'. Expected headers: {}",
                    expected_header,
                    expected_headers.join(", ")
                ),
            });
        }
    }

    if !errors.is_empty() {
        return CsvParseResult {
            validated_rows,
            errors,
            total_rows: 0,
        };
    }

    // Added: Map column indices from header names for flexible column ordering
    let email_idx = header_names.iter().position(|h| *h == "email").unwrap();
    let display_name_idx = header_names.iter().position(|h| *h == "display_name").unwrap();
    let password_idx = header_names.iter().position(|h| *h == "password").unwrap();
    let role_idx = header_names.iter().position(|h| *h == "role").unwrap();

    let mut row_number = 1_usize; // NOTE: 1-indexed to match human-readable CSV line numbers (after header)

    for result in reader.records() {
        row_number += 1;
        match result {
            Ok(record) => {
                let email = record.get(email_idx).unwrap_or("").trim().to_string();
                let display_name = record.get(display_name_idx).unwrap_or("").trim().to_string();
                let password = record.get(password_idx).unwrap_or("").trim().to_string();
                let role = record.get(role_idx).unwrap_or("").trim().to_lowercase();

                let row_errors = validate_row(row_number, &email, &display_name, &password, &role);

                if row_errors.is_empty() {
                    validated_rows.push(BulkImportRow {
                        email,
                        display_name,
                        password,
                        role,
                    });
                } else {
                    errors.extend(row_errors);
                }
            }
            Err(csv_error) => {
                errors.push(BulkImportError {
                    row: row_number,
                    field: "row".to_string(),
                    message: format!("Failed to parse row: {}", csv_error),
                });
            }
        }
    }

    // NOTE: total_rows is data rows (excluding header), which is row_number - 1
    let total_rows = row_number - 1;

    CsvParseResult {
        validated_rows,
        errors,
        total_rows,
    }
}

/// PURPOSE: Basic email format validation (local@domain.tld)
/// CONSTRAINTS: Not a full RFC 5322 check — validates structure only
fn is_valid_email(email: &str) -> bool {
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 {
        return false;
    }
    let local = parts[0];
    let domain = parts[1];
    // Added: Local part must be non-empty, domain must contain a dot and non-empty segments
    if local.is_empty() || domain.is_empty() {
        return false;
    }
    let domain_parts: Vec<&str> = domain.split('.').collect();
    domain_parts.len() >= 2 && domain_parts.iter().all(|segment| !segment.is_empty())
}

/// PURPOSE: Validate individual fields of a parsed CSV row
/// CONSTRAINTS: Returns all errors for the row (not just the first one)
fn validate_row(
    row: usize,
    email: &str,
    display_name: &str,
    password: &str,
    role: &str,
) -> Vec<BulkImportError> {
    let mut errors = Vec::new();

    // Added: Email validation — basic format check
    if email.is_empty() {
        errors.push(BulkImportError {
            row,
            field: "email".to_string(),
            message: "Email is required".to_string(),
        });
    } else if !is_valid_email(email) {
        errors.push(BulkImportError {
            row,
            field: "email".to_string(),
            message: format!("Invalid email format: '{}'", email),
        });
    }

    // Added: Display name validation
    if display_name.is_empty() {
        errors.push(BulkImportError {
            row,
            field: "display_name".to_string(),
            message: "Display name is required".to_string(),
        });
    }

    // Added: Password validation — minimum 8 characters
    if password.is_empty() {
        errors.push(BulkImportError {
            row,
            field: "password".to_string(),
            message: "Password is required".to_string(),
        });
    } else if password.len() < 8 {
        errors.push(BulkImportError {
            row,
            field: "password".to_string(),
            message: format!(
                "Password must be at least 8 characters (got {})",
                password.len()
            ),
        });
    }

    // Added: Role validation — must be 'user' or 'admin'
    if role.is_empty() {
        errors.push(BulkImportError {
            row,
            field: "role".to_string(),
            message: "Role is required".to_string(),
        });
    } else if role != "user" && role != "admin" {
        errors.push(BulkImportError {
            row,
            field: "role".to_string(),
            message: format!("Role must be 'user' or 'admin', got '{}'", role),
        });
    }

    errors
}

/// PURPOSE: Generate a CSV template string for download
pub fn generate_template() -> String {
    "email,display_name,password,role\nuser@example.com,John Doe,SecurePass123!,user\n".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_csv() {
        // Added: Verify parsing a well-formed CSV with valid rows
        let csv_content = "email,display_name,password,role\njane@example.com,Jane Doe,Password123!,user\njohn@example.com,John Smith,Admin@Pass99,admin\n";
        let result = parse_csv(csv_content);

        assert_eq!(result.total_rows, 2);
        assert_eq!(result.validated_rows.len(), 2);
        assert!(result.errors.is_empty());
        assert_eq!(result.validated_rows[0].email, "jane@example.com");
        assert_eq!(result.validated_rows[0].role, "user");
        assert_eq!(result.validated_rows[1].email, "john@example.com");
        assert_eq!(result.validated_rows[1].role, "admin");
    }

    #[test]
    fn test_parse_csv_with_validation_errors() {
        // Added: Verify rows with invalid data produce errors
        let csv_content = "email,display_name,password,role\nbad-email,Jane Doe,Password123!,user\njane@example.com,,Password123!,user\njane@example.com,Jane Doe,short,user\njane@example.com,Jane Doe,Password123!,superadmin\n";
        let result = parse_csv(csv_content);

        assert_eq!(result.total_rows, 4);
        assert_eq!(result.validated_rows.len(), 0);
        assert_eq!(result.errors.len(), 4);

        // NOTE: Row 2 = bad email, row 3 = empty display_name, row 4 = short password, row 5 = invalid role
        assert_eq!(result.errors[0].field, "email");
        assert_eq!(result.errors[1].field, "display_name");
        assert_eq!(result.errors[2].field, "password");
        assert_eq!(result.errors[3].field, "role");
    }

    #[test]
    fn test_parse_empty_csv() {
        // Added: Verify empty input produces file-level error
        let result = parse_csv("");
        assert_eq!(result.total_rows, 0);
        assert!(result.validated_rows.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].field, "file");
    }

    #[test]
    fn test_parse_csv_missing_headers() {
        // Added: Verify CSV with wrong headers produces header error
        let csv_content = "name,address\nJane,123 Main St\n";
        let result = parse_csv(csv_content);

        assert!(result.validated_rows.is_empty());
        assert!(!result.errors.is_empty());
        assert_eq!(result.errors[0].field, "headers");
    }

    #[test]
    fn test_parse_csv_partial_headers() {
        // Added: Verify CSV with some but not all required headers
        let csv_content = "email,display_name\njane@example.com,Jane\n";
        let result = parse_csv(csv_content);

        assert!(result.validated_rows.is_empty());
        // NOTE: Should report missing 'password' and 'role' headers
        assert!(result.errors.len() >= 2);
    }

    #[test]
    fn test_parse_csv_with_whitespace_trimming() {
        // Added: Verify whitespace around values is trimmed
        let csv_content = "email,display_name,password,role\n  alice@example.com , Alice  ,  Password123!  , user \n";
        let result = parse_csv(csv_content);

        assert_eq!(result.validated_rows.len(), 1);
        assert_eq!(result.validated_rows[0].email, "alice@example.com");
        assert_eq!(result.validated_rows[0].display_name, "Alice");
        assert_eq!(result.validated_rows[0].role, "user");
    }

    #[test]
    fn test_parse_csv_role_case_insensitive() {
        // Added: Verify role is normalized to lowercase
        let csv_content = "email,display_name,password,role\nalice@example.com,Alice,Password123!,USER\nbob@example.com,Bob,Password123!,Admin\n";
        let result = parse_csv(csv_content);

        assert_eq!(result.validated_rows.len(), 2);
        assert_eq!(result.validated_rows[0].role, "user");
        assert_eq!(result.validated_rows[1].role, "admin");
    }

    #[test]
    fn test_parse_csv_mixed_valid_and_invalid() {
        // Added: Verify valid rows are accepted even when other rows have errors
        let csv_content = "email,display_name,password,role\nalice@example.com,Alice,Password123!,user\nbad-email,,short,unknown\nbob@example.com,Bob,StrongP@ss1,admin\n";
        let result = parse_csv(csv_content);

        assert_eq!(result.total_rows, 3);
        assert_eq!(result.validated_rows.len(), 2);
        // NOTE: Row 3 (bad-email) has 4 errors: email, display_name, password, role
        assert_eq!(result.errors.len(), 4);
    }

    #[test]
    fn test_email_validation_rules() {
        // Added: Verify email validation catches various invalid formats
        let invalid_emails = ["", "noatsign", "no@dot", "@nodomain.com"];
        for invalid_email in invalid_emails {
            let row_errors = validate_row(1, invalid_email, "Name", "Password123!", "user");
            assert!(
                !row_errors.is_empty(),
                "Expected error for email '{}'",
                invalid_email
            );
        }

        let valid_emails = ["user@example.com", "admin@mail.org", "test.user@sub.domain.com"];
        for valid_email in valid_emails {
            let row_errors = validate_row(1, valid_email, "Name", "Password123!", "user");
            assert!(
                row_errors.is_empty(),
                "Expected no error for email '{}', got {:?}",
                valid_email,
                row_errors.iter().map(|e| &e.message).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn test_password_length_validation() {
        // Added: Verify password minimum length of 8 characters
        let short_password_errors = validate_row(1, "u@e.com", "Name", "1234567", "user");
        assert_eq!(short_password_errors.len(), 1);
        assert_eq!(short_password_errors[0].field, "password");

        let valid_password_errors = validate_row(1, "u@e.com", "Name", "12345678", "user");
        assert!(valid_password_errors.is_empty());
    }

    #[test]
    fn test_generate_template() {
        // Added: Verify template has correct headers and example row
        let template = generate_template();
        assert!(template.starts_with("email,display_name,password,role"));
        assert!(template.contains("user@example.com"));
        assert!(template.contains("John Doe"));
    }
}
