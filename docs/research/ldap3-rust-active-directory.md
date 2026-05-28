# LDAP3 Rust crate — Active Directory integration research (TMAIL-100)

Captured: 2026-05-28
Trigger: TMAIL-100 auto-fix worker session implementing LDAP/AD user sync.

## Crate selection

**Chosen:** [`ldap3`](https://crates.io/crates/ldap3) v0.12.x.

- Pure-Rust, Tokio-based async LDAP client.
- Cross-platform Kerberos/GSSAPI support since 0.10.3.
- Basic NTLM support in the 0.12 branch (TLS connections, or clear connections
  without signing/sealing).
- Requires Rust ≥1.85 with NTLM enabled, ≥1.82 without. Our backend is on
  edition 2024 (Rust 1.85+), so we are compatible.

Discarded alternatives:
- `simple-ldap` — thin wrapper around `ldap3`, no additional value for our use case.
- Other forks (`learnrust/ldap3`, `svend/ldap3`, `Firstyear/ldap3`) — mirrors,
  upstream `inejge/ldap3` is canonical.

## Key API surface

```rust
use ldap3::{LdapConnAsync, Scope, SearchEntry};

let (conn, mut ldap) = LdapConnAsync::new("ldaps://ad.example.com:636").await?;
ldap3::drive!(conn); // spawns the background IO driver task

ldap.simple_bind("cn=svc,dc=example,dc=com", "password").await?.success()?;

let (rs, _res) = ldap
    .search(
        "ou=Users,dc=example,dc=com",
        Scope::Subtree,
        "(&(objectClass=user)(mail=*))",
        vec!["mail", "displayName", "sAMAccountName", "memberOf"],
    )
    .await?
    .success()?;

for entry in rs {
    let se = SearchEntry::construct(entry);
    let email = se.attrs.get("mail").and_then(|v| v.first()).cloned();
    let groups = se.attrs.get("memberOf").cloned().unwrap_or_default();
    // ... map into TASMail user
}

ldap.unbind().await?;
```

## Attribute mapping (AD ↔ TASMail)

| LDAP/AD attribute | TASMail field | Notes |
|---|---|---|
| `mail` (or `userPrincipalName`) | `users.email` | Primary key for match. Configurable via `email_attribute`. |
| `displayName` | `users.display_name` | Configurable via `name_attribute`. |
| `sAMAccountName` | `users.username` | Pre-Windows-2000 login name; unique per domain. |
| `userAccountControl` bit 0x2 | `users.disabled` | `ACCOUNTDISABLE` flag — when set, user should be disabled in TASMail. |
| `memberOf` | distribution group membership | Multi-valued; each value is a group DN. |

References:
- [Microsoft userAccountControl flags](https://learn.microsoft.com/en-us/troubleshoot/windows-server/active-directory/useraccountcontrol-manipulate-account-properties)
- [Microsoft Dovecot AD howto](https://doc.dovecot.org/main/howto/active_directory.html)

## Sync algorithm

For each row returned by the configured search:

1. Build `(email, display_name, sam_account_name, is_disabled, groups)` tuple.
2. If `email` is missing, log to `errors[]` and skip (cannot create TASMail
   account without an address).
3. Look up `users` by `email`:
   - **No row** → INSERT (created++). Default password is a random sentinel that
     can never satisfy LDAP — auth path falls back to LDAP bind.
   - **Row exists** and either name/display changed → UPDATE (updated++).
   - **Row exists** and LDAP says disabled → soft-disable in TASMail (disabled++).
4. Accumulate counts and errors, persist to `ldap_sync_logs`.

## Scope deferred for follow-on tickets

The full spec for TMAIL-100 also calls for:

- **Dovecot LDAP passdb** — `dovecot-ldap.conf.ext` for direct IMAP auth. Not
  installed on `mail.techatscale.io` (BYOK product); ship as deploy artefact only.
- **Periodic sync job** — background tokio task polling each config every
  `sync_interval_minutes`. Belongs alongside `email_scheduler` in `services/`.
- **Real-time LDAP auth on login** — TASMail's `POST /api/auth/login` should
  attempt LDAP simple bind against any matching enabled LDAP config before
  falling back to the local Argon2id hash.
- **Group sync** — multi-valued `memberOf` → rows in `groups` /
  `group_members`. Touches the contacts/groups domain, not just users.

Each of these is implementation-distinct enough to be its own TMAIL ticket.

## Sources

- [inejge/ldap3 (canonical upstream)](https://github.com/inejge/ldap3)
- [docs.rs/ldap3 latest](https://docs.rs/ldap3)
- [docs.rs/ldap3 v0.12.1](https://docs.rs/crate/ldap3/latest)
- [crates.io/crates/ldap3](https://crates.io/crates/ldap3)
- [LdapConnAsync API](https://docs.rs/ldap3/latest/ldap3/struct.LdapConnAsync.html)
- [LdapConn API](https://docs.rs/ldap3/latest/ldap3/struct.LdapConn.html)
- [lib.rs/crates/ldap3](https://lib.rs/crates/ldap3)
- [Rust forum LDAP discussion](https://users.rust-lang.org/t/ldap-library-for-rust/10449)
- [Dovecot AD howto](https://doc.dovecot.org/main/howto/active_directory.html)
- [Microsoft userAccountControl reference](https://learn.microsoft.com/en-us/troubleshoot/windows-server/active-directory/useraccountcontrol-manipulate-account-properties)
