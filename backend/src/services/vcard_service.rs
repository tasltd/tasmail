// Added: vCard import/export service for contact management (TMAIL-119)
use serde::{Deserialize, Serialize};

// PURPOSE: Represents a contact parsed from vCard text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactImport {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub company: Option<String>,
}

// PURPOSE: Parse vCard 3.0/4.0 text into a list of ContactImport structs
pub fn parse_vcard(vcard_text: &str) -> Vec<ContactImport> {
    let mut contacts = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_email: Option<String> = None;
    let mut current_phone: Option<String> = None;
    let mut current_company: Option<String> = None;
    let mut in_vcard = false;

    for line in vcard_text.lines() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case("BEGIN:VCARD") {
            in_vcard = true;
            current_name = None;
            current_email = None;
            current_phone = None;
            current_company = None;
            continue;
        }

        if trimmed.eq_ignore_ascii_case("END:VCARD") {
            // NOTE: Only add contacts that have at least an email
            if in_vcard {
                if let Some(email) = current_email.take() {
                    contacts.push(ContactImport {
                        name: current_name.take().unwrap_or_else(|| email.clone()),
                        email,
                        phone: current_phone.take(),
                        company: current_company.take(),
                    });
                }
            }
            in_vcard = false;
            continue;
        }

        if !in_vcard {
            continue;
        }

        // NOTE: Handle property lines — split on first ':'
        if let Some(colon_pos) = trimmed.find(':') {
            let prop_part = &trimmed[..colon_pos];
            let value = trimmed[colon_pos + 1..].trim();

            // NOTE: Property name is before any ';' params (e.g., "TEL;TYPE=WORK")
            let prop_name = prop_part
                .split(';')
                .next()
                .unwrap_or("")
                .to_uppercase();

            match prop_name.as_str() {
                "FN" => {
                    if !value.is_empty() {
                        current_name = Some(value.to_string());
                    }
                }
                "EMAIL" => {
                    if !value.is_empty() {
                        current_email = Some(value.to_string());
                    }
                }
                "TEL" => {
                    if !value.is_empty() {
                        current_phone = Some(value.to_string());
                    }
                }
                "ORG" => {
                    // NOTE: ORG can have multiple components separated by ';'
                    if !value.is_empty() {
                        let org = value.split(';').next().unwrap_or(value).to_string();
                        current_company = Some(org);
                    }
                }
                _ => {}
            }
        }
    }

    contacts
}

// PURPOSE: Export a list of contacts into vCard 3.0 format text
pub fn export_vcard(contacts: &[ContactExport]) -> String {
    let mut output = String::new();

    for contact in contacts {
        output.push_str("BEGIN:VCARD\r\n");
        output.push_str("VERSION:3.0\r\n");
        output.push_str(&format!("FN:{}\r\n", contact.display_name.as_deref().unwrap_or(&contact.email)));
        output.push_str(&format!("EMAIL:{}\r\n", contact.email));
        if let Some(ref phone) = contact.phone {
            output.push_str(&format!("TEL:{}\r\n", phone));
        }
        if let Some(ref company) = contact.company {
            output.push_str(&format!("ORG:{}\r\n", company));
        }
        output.push_str("END:VCARD\r\n");
    }

    output
}

// PURPOSE: Struct for exporting existing contacts to vCard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactExport {
    pub email: String,
    pub display_name: Option<String>,
    pub phone: Option<String>,
    pub company: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_vcard() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice Smith\r\nEMAIL:alice@example.com\r\nTEL:+233201234567\r\nORG:Acme Corp\r\nEND:VCARD";
        let contacts = parse_vcard(text);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name, "Alice Smith");
        assert_eq!(contacts[0].email, "alice@example.com");
        assert_eq!(contacts[0].phone.as_deref(), Some("+233201234567"));
        assert_eq!(contacts[0].company.as_deref(), Some("Acme Corp"));
    }

    #[test]
    fn test_parse_multiple_vcards() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice\r\nEMAIL:alice@test.com\r\nEND:VCARD\r\nBEGIN:VCARD\r\nVERSION:4.0\r\nFN:Bob\r\nEMAIL:bob@test.com\r\nEND:VCARD";
        let contacts = parse_vcard(text);
        assert_eq!(contacts.len(), 2);
        assert_eq!(contacts[0].name, "Alice");
        assert_eq!(contacts[1].name, "Bob");
    }

    #[test]
    fn test_parse_vcard_minimal() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nEMAIL:min@test.com\r\nEND:VCARD";
        let contacts = parse_vcard(text);
        assert_eq!(contacts.len(), 1);
        // NOTE: Name defaults to email when FN is absent
        assert_eq!(contacts[0].name, "min@test.com");
        assert_eq!(contacts[0].email, "min@test.com");
        assert!(contacts[0].phone.is_none());
        assert!(contacts[0].company.is_none());
    }

    #[test]
    fn test_parse_vcard_no_email_skipped() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:No Email\r\nEND:VCARD";
        let contacts = parse_vcard(text);
        assert!(contacts.is_empty());
    }

    #[test]
    fn test_parse_vcard_with_params() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Jane\r\nEMAIL;TYPE=WORK:jane@work.com\r\nTEL;TYPE=CELL:+1234567890\r\nEND:VCARD";
        let contacts = parse_vcard(text);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].email, "jane@work.com");
        assert_eq!(contacts[0].phone.as_deref(), Some("+1234567890"));
    }

    #[test]
    fn test_parse_vcard_org_with_semicolons() {
        let text = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:John\r\nEMAIL:john@test.com\r\nORG:Big Corp;Engineering\r\nEND:VCARD";
        let contacts = parse_vcard(text);
        assert_eq!(contacts.len(), 1);
        // NOTE: Only first ORG component is taken as company name
        assert_eq!(contacts[0].company.as_deref(), Some("Big Corp"));
    }

    #[test]
    fn test_parse_empty_string() {
        let contacts = parse_vcard("");
        assert!(contacts.is_empty());
    }

    #[test]
    fn test_parse_vcard_case_insensitive() {
        let text = "begin:vcard\r\nversion:3.0\r\nfn:Test\r\nemail:test@test.com\r\nend:vcard";
        let contacts = parse_vcard(text);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name, "Test");
    }

    #[test]
    fn test_export_single_contact() {
        let contacts = vec![ContactExport {
            email: "alice@example.com".to_string(),
            display_name: Some("Alice Smith".to_string()),
            phone: Some("+233201234567".to_string()),
            company: Some("Acme Corp".to_string()),
        }];

        let vcard = export_vcard(&contacts);
        assert!(vcard.contains("BEGIN:VCARD"));
        assert!(vcard.contains("VERSION:3.0"));
        assert!(vcard.contains("FN:Alice Smith"));
        assert!(vcard.contains("EMAIL:alice@example.com"));
        assert!(vcard.contains("TEL:+233201234567"));
        assert!(vcard.contains("ORG:Acme Corp"));
        assert!(vcard.contains("END:VCARD"));
    }

    #[test]
    fn test_export_contact_no_optional_fields() {
        let contacts = vec![ContactExport {
            email: "bob@test.com".to_string(),
            display_name: None,
            phone: None,
            company: None,
        }];

        let vcard = export_vcard(&contacts);
        assert!(vcard.contains("FN:bob@test.com"));
        assert!(vcard.contains("EMAIL:bob@test.com"));
        assert!(!vcard.contains("TEL:"));
        assert!(!vcard.contains("ORG:"));
    }

    #[test]
    fn test_export_multiple_contacts() {
        let contacts = vec![
            ContactExport {
                email: "a@test.com".to_string(),
                display_name: Some("A".to_string()),
                phone: None,
                company: None,
            },
            ContactExport {
                email: "b@test.com".to_string(),
                display_name: Some("B".to_string()),
                phone: None,
                company: None,
            },
        ];

        let vcard = export_vcard(&contacts);
        // NOTE: Should contain two complete vCard blocks
        let begin_count = vcard.matches("BEGIN:VCARD").count();
        let end_count = vcard.matches("END:VCARD").count();
        assert_eq!(begin_count, 2);
        assert_eq!(end_count, 2);
    }

    #[test]
    fn test_roundtrip_vcard() {
        let contacts = vec![ContactExport {
            email: "round@trip.com".to_string(),
            display_name: Some("Round Trip".to_string()),
            phone: Some("+5551234".to_string()),
            company: Some("TripCo".to_string()),
        }];

        let exported = export_vcard(&contacts);
        let imported = parse_vcard(&exported);

        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].name, "Round Trip");
        assert_eq!(imported[0].email, "round@trip.com");
        assert_eq!(imported[0].phone.as_deref(), Some("+5551234"));
        assert_eq!(imported[0].company.as_deref(), Some("TripCo"));
    }

    #[test]
    fn test_parse_vcard_with_unix_newlines() {
        let text = "BEGIN:VCARD\nVERSION:3.0\nFN:Unix\nEMAIL:unix@test.com\nEND:VCARD";
        let contacts = parse_vcard(text);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name, "Unix");
    }
}
