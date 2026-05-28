// Added: Real LDAP/AD directory query and sync logic for TMAIL-100.
//
// PURPOSE: Encapsulate all `ldap3` interaction in one place so the handler
// layer stays thin and unit tests can target the attribute-mapping pure
// functions without needing a live directory.
//
// SCOPE for this iteration:
//   * `test_connection`     — bind to verify credentials (used by the
//                             POST /api/admin/ldap/:id/test endpoint).
//   * `search_users`        — bind, run the configured search, return a
//                             Vec<LdapUser> with mapped attributes.
//   * `apply_sync`          — given a Vec<LdapUser>, create / update /
//                             soft-disable rows in `mailboxes`.
//
// DEFERRED (each is its own TMAIL ticket — see CLAUDE.md + research doc):
//   * Periodic background scheduler that calls `search_users` + `apply_sync`
//     on `sync_interval_minutes`.
//   * Real-time LDAP simple-bind on POST /api/auth/login.
//   * Dovecot `dovecot-ldap.conf.ext` passdb shipped as a deploy artefact.
//   * Group sync (memberOf → distribution groups in `groups` table).

use std::collections::HashMap;

use ldap3::{LdapConnAsync, Scope, SearchEntry};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::auth_service::hash_password;

/// PURPOSE: A single LDAP directory user mapped onto TASMail-relevant fields.
/// CONSTRAINTS: `email` is the only field guaranteed present — everything else
/// is best-effort because directory schemas vary.
#[derive(Debug, Clone, PartialEq)]
pub struct LdapUser {
    pub email: String,
    pub display_name: Option<String>,
    pub sam_account_name: Option<String>,
    pub is_disabled: bool,
    pub groups: Vec<String>,
}

/// PURPOSE: Result counts + per-row errors from a single `apply_sync` call.
/// CONSTRAINTS: `errors` is JSON-serialisable so the handler can persist it
/// to `ldap_sync_logs.errors` as-is.
#[derive(Debug, Default, Clone)]
pub struct LdapSyncResult {
    pub created: i32,
    pub updated: i32,
    pub disabled: i32,
    pub errors: Vec<serde_json::Value>,
}

pub struct LdapService;

impl LdapService {
    /// PURPOSE: Verify the configured service account can bind. No search is
    /// performed — this is the cheapest possible health-check.
    pub async fn test_connection(
        server_url: &str,
        bind_dn: &str,
        bind_password: &str,
    ) -> anyhow::Result<()> {
        let (conn, mut ldap) = LdapConnAsync::new(server_url).await?;
        ldap3::drive!(conn);
        ldap.simple_bind(bind_dn, bind_password).await?.success()?;
        // NOTE: ignore the error from unbind — connection will close anyway.
        let _ = ldap.unbind().await;
        Ok(())
    }

    /// PURPOSE: Bind, run the configured search, return mapped users.
    pub async fn search_users(
        server_url: &str,
        bind_dn: &str,
        bind_password: &str,
        search_base: &str,
        search_filter: &str,
        email_attribute: &str,
        name_attribute: &str,
    ) -> anyhow::Result<Vec<LdapUser>> {
        let (conn, mut ldap) = LdapConnAsync::new(server_url).await?;
        ldap3::drive!(conn);
        ldap.simple_bind(bind_dn, bind_password).await?.success()?;

        // NOTE: ask for the standard AD-flavoured attribute set. `memberOf`
        // is multi-valued; `userAccountControl` is a bit-flag string.
        let attrs = vec![
            email_attribute,
            name_attribute,
            "sAMAccountName",
            "userAccountControl",
            "memberOf",
        ];
        let (rs, _res) = ldap
            .search(search_base, Scope::Subtree, search_filter, attrs)
            .await?
            .success()?;

        let mut users = Vec::with_capacity(rs.len());
        for entry in rs {
            let se = SearchEntry::construct(entry);
            if let Some(u) = map_search_entry(&se.attrs, email_attribute, name_attribute) {
                users.push(u);
            }
        }

        let _ = ldap.unbind().await;
        Ok(users)
    }

    /// PURPOSE: Apply a list of directory users to TASMail's `mailboxes` table.
    /// CONSTRAINTS: We never panic on a per-row failure — bad rows go into
    /// `errors[]` and the sync still reports a partial-success count.
    pub async fn apply_sync(pool: &PgPool, users: Vec<LdapUser>) -> LdapSyncResult {
        let mut result = LdapSyncResult::default();

        // NOTE: bulk_import uses the same "first domain" rule for default mapping;
        // a future multi-domain ticket can split on the email's `@` segment.
        let default_domain_id = match fetch_default_domain_id(pool).await {
            Ok(id) => id,
            Err(e) => {
                result.errors.push(json!({
                    "error": format!("no default domain available: {e}"),
                }));
                return result;
            }
        };

        for u in users {
            match sync_single_user(pool, &u, default_domain_id).await {
                Ok(SyncOutcome::Created) => result.created += 1,
                Ok(SyncOutcome::Updated) => result.updated += 1,
                Ok(SyncOutcome::Disabled) => result.disabled += 1,
                Ok(SyncOutcome::Unchanged) => {}
                Err(e) => result.errors.push(json!({
                    "email": u.email,
                    "error": e.to_string(),
                })),
            }
        }

        result
    }
}

enum SyncOutcome {
    Created,
    Updated,
    Disabled,
    Unchanged,
}

async fn fetch_default_domain_id(pool: &PgPool) -> anyhow::Result<Uuid> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM domains ORDER BY created_at LIMIT 1")
            .fetch_optional(pool)
            .await?;
    row.map(|(id,)| id).ok_or_else(|| {
        anyhow::anyhow!("no domain configured — create one before running LDAP sync")
    })
}

async fn sync_single_user(
    pool: &PgPool,
    u: &LdapUser,
    default_domain_id: Uuid,
) -> anyhow::Result<SyncOutcome> {
    // NOTE: query ignores the `active = true` filter that `Mailbox::find_by_username`
    // applies — we need to see disabled rows so we can re-enable them.
    let existing: Option<(Uuid, Option<String>, bool)> = sqlx::query_as(
        "SELECT id, display_name, active FROM mailboxes WHERE username = $1",
    )
    .bind(&u.email)
    .fetch_optional(pool)
    .await?;

    match existing {
        None => {
            if u.is_disabled {
                // NOTE: don't create disabled accounts — saves disk and avoids confusing the admin.
                return Ok(SyncOutcome::Unchanged);
            }
            // NOTE: generate a random non-bindable password hash. The real-time
            // LDAP auth ticket will plug bind-on-login in; until then an admin
            // must manually set a local password.
            let sentinel = hash_password(&format!("ldap-sync-sentinel-{}", Uuid::new_v4()))
                .map_err(|e| anyhow::anyhow!("hash failed: {e}"))?;
            sqlx::query(
                "INSERT INTO mailboxes
                   (id, domain_id, username, password_hash, display_name,
                    quota_bytes, active, is_admin, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, true, false, NOW(), NOW())",
            )
            .bind(Uuid::new_v4())
            .bind(default_domain_id)
            .bind(&u.email)
            .bind(&sentinel)
            .bind(u.display_name.as_deref())
            .bind(1_073_741_824_i64)
            .execute(pool)
            .await?;
            Ok(SyncOutcome::Created)
        }
        Some((id, current_name, current_active)) => {
            if u.is_disabled && current_active {
                sqlx::query("UPDATE mailboxes SET active = false, updated_at = NOW() WHERE id = $1")
                    .bind(id)
                    .execute(pool)
                    .await?;
                return Ok(SyncOutcome::Disabled);
            }
            if !u.is_disabled && (!current_active || current_name.as_deref() != u.display_name.as_deref()) {
                sqlx::query(
                    "UPDATE mailboxes SET
                        display_name = $2,
                        active = true,
                        updated_at = NOW()
                     WHERE id = $1",
                )
                .bind(id)
                .bind(u.display_name.as_deref())
                .execute(pool)
                .await?;
                return Ok(SyncOutcome::Updated);
            }
            Ok(SyncOutcome::Unchanged)
        }
    }
}

/// PURPOSE: Pull the first value out of a multi-valued LDAP attribute. Returned
/// as an owned String so the caller can store it.
pub fn extract_first(attrs: &HashMap<String, Vec<String>>, key: &str) -> Option<String> {
    attrs.get(key).and_then(|v| v.first().cloned())
}

/// PURPOSE: Interpret Active Directory's `userAccountControl` bit-flag string.
/// The `ACCOUNTDISABLE` bit (0x2) is what we care about for the sync.
/// Returns true when the account is flagged disabled.
pub fn parse_user_account_control_disabled(raw: Option<&str>) -> bool {
    raw.and_then(|s| s.parse::<u32>().ok())
        .map(|flags| flags & 0x2 != 0)
        .unwrap_or(false)
}

/// PURPOSE: Build an `LdapUser` from a single search entry's attribute map.
/// Returns `None` when the email attribute is missing — those rows are
/// skipped by `search_users` because we can't key off them.
pub fn map_search_entry(
    attrs: &HashMap<String, Vec<String>>,
    email_attribute: &str,
    name_attribute: &str,
) -> Option<LdapUser> {
    let email = extract_first(attrs, email_attribute)?;
    if email.trim().is_empty() {
        return None;
    }
    Some(LdapUser {
        email: email.to_lowercase(),
        display_name: extract_first(attrs, name_attribute),
        sam_account_name: extract_first(attrs, "sAMAccountName"),
        is_disabled: parse_user_account_control_disabled(
            attrs
                .get("userAccountControl")
                .and_then(|v| v.first())
                .map(String::as_str),
        ),
        groups: attrs.get("memberOf").cloned().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(k, vs)| (k.to_string(), vs.iter().map(|s| s.to_string()).collect()))
            .collect()
    }

    #[test]
    fn extract_first_returns_first_value() {
        let a = attrs(&[("memberOf", &["CN=A,DC=corp", "CN=B,DC=corp"])]);
        assert_eq!(extract_first(&a, "memberOf").as_deref(), Some("CN=A,DC=corp"));
    }

    #[test]
    fn extract_first_returns_none_when_missing() {
        let a = attrs(&[("mail", &["x@y"])]);
        assert!(extract_first(&a, "missing").is_none());
    }

    #[test]
    fn extract_first_returns_none_when_attr_value_list_empty() {
        let mut a = HashMap::new();
        a.insert("mail".to_string(), Vec::<String>::new());
        assert!(extract_first(&a, "mail").is_none());
    }

    #[test]
    fn uac_disabled_bit_detected() {
        // 0x2 (ACCOUNTDISABLE) | 0x200 (NORMAL_ACCOUNT) = 514
        assert!(parse_user_account_control_disabled(Some("514")));
    }

    #[test]
    fn uac_enabled_account_not_flagged_disabled() {
        // 0x200 only — NORMAL_ACCOUNT, enabled
        assert!(!parse_user_account_control_disabled(Some("512")));
    }

    #[test]
    fn uac_none_or_garbage_treated_as_enabled() {
        assert!(!parse_user_account_control_disabled(None));
        assert!(!parse_user_account_control_disabled(Some("not-a-number")));
        assert!(!parse_user_account_control_disabled(Some("")));
    }

    #[test]
    fn map_search_entry_full_ad_row() {
        let a = attrs(&[
            ("mail", &["KWAME@CORP.GH"]),
            ("displayName", &["Kwame Mensah"]),
            ("sAMAccountName", &["kmensah"]),
            ("userAccountControl", &["512"]),
            ("memberOf", &["CN=Mail,OU=Groups,DC=corp,DC=gh", "CN=Staff,OU=Groups,DC=corp,DC=gh"]),
        ]);
        let u = map_search_entry(&a, "mail", "displayName").expect("should map");
        assert_eq!(u.email, "kwame@corp.gh"); // lower-cased
        assert_eq!(u.display_name.as_deref(), Some("Kwame Mensah"));
        assert_eq!(u.sam_account_name.as_deref(), Some("kmensah"));
        assert!(!u.is_disabled);
        assert_eq!(u.groups.len(), 2);
    }

    #[test]
    fn map_search_entry_disabled_account() {
        let a = attrs(&[
            ("mail", &["ex@corp.gh"]),
            ("userAccountControl", &["514"]), // 512 + 2
        ]);
        let u = map_search_entry(&a, "mail", "displayName").unwrap();
        assert!(u.is_disabled);
        assert!(u.display_name.is_none());
    }

    #[test]
    fn map_search_entry_skips_missing_email() {
        let a = attrs(&[("displayName", &["No Email Here"])]);
        assert!(map_search_entry(&a, "mail", "displayName").is_none());
    }

    #[test]
    fn map_search_entry_skips_empty_email() {
        let a = attrs(&[("mail", &["   "]), ("displayName", &["x"])]);
        assert!(map_search_entry(&a, "mail", "displayName").is_none());
    }

    #[test]
    fn map_search_entry_respects_configurable_email_attribute() {
        // Many AD deployments use userPrincipalName instead of mail.
        let a = attrs(&[
            ("userPrincipalName", &["upn@corp.gh"]),
            ("cn", &["UPN User"]),
        ]);
        let u = map_search_entry(&a, "userPrincipalName", "cn").unwrap();
        assert_eq!(u.email, "upn@corp.gh");
        assert_eq!(u.display_name.as_deref(), Some("UPN User"));
    }

    #[test]
    fn sync_result_default_is_zero_zero_zero() {
        let r = LdapSyncResult::default();
        assert_eq!(r.created, 0);
        assert_eq!(r.updated, 0);
        assert_eq!(r.disabled, 0);
        assert!(r.errors.is_empty());
    }
}
