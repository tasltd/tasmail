# RustMail — Project Team Members

**Project:** RustMail Self-Hosted Email Service
**Institution:** University of Ghana
**Repository:** https://github.com/dfdodzi/project-email-service (private)
**Updated:** 2026-03-07

---

## Team Roster

| # | Name | Student ID | Email | GitHub Username | Role | Invitation |
|---|------|-----------|-------|----------------|------|------------|
| 1 | Dominic Fui Nusenu Dodzi | 22103426 | dfdodzi-nusenu@st.ug.edu.gh | `dfdodzi` | **Owner** | N/A |
| 2 | Benard Obeng Akoto | 22012184 | baobeng019@st.ug.edu.gh | — | Collaborator (write) | Pending — needs GitHub account |
| 3 | Stephen Nyarko | 22247080 | snyarko055@st.ug.edu.gh | `MhanDEV` | Collaborator (write) | Sent |
| 4 | Gabriel Neequaye | 22152238 | gneequaye006@st.ug.edu.gh | — | Collaborator (write) | Pending — needs GitHub account |
| 5 | Wisdom Sabadu | 22102174 | wsabadu@st.ug.edu.gh | `wisdom-Git` | Collaborator (write) | Sent |
| 6 | Richmond Sagoe | 22017399 | rsagoe009@st.ug.edu.gh | `RichmondSagoe` | Collaborator (write) | Sent |
| 7 | Christiana Amoak Abisitemi | 22032136 | caamoak@st.ug.edu.gh | — | Collaborator (write) | Pending — needs GitHub account |

---

## Additional Accounts

| Account | Email | GitHub Username | Role | Notes |
|---------|-------|----------------|------|-------|
| Dominic (secondary) | dfdnusenu@gmail.com | `dfdnusenu` | Admin | Pending — GitHub account does not exist yet |
| daptordarattler | dfdnusenu@gmail.com | `daptordarattler` | Admin | CI/CD and automation account |

---

## Invitation Status Summary

**Invited (3):** Stephen Nyarko (`MhanDEV`), Wisdom Sabadu (`wisdom-Git`), Richmond Sagoe (`RichmondSagoe`) — must accept invitation via GitHub notification or email.

**Pending GitHub account creation (4):**
- Benard Obeng Akoto (baobeng019@st.ug.edu.gh)
- Gabriel Neequaye (gneequaye006@st.ug.edu.gh)
- Christiana Amoak Abisitemi (caamoak@st.ug.edu.gh)
- dfdnusenu (dfdnusenu@gmail.com) — admin role

These members must first create a GitHub account at https://github.com/signup (recommend using their `@st.ug.edu.gh` email for GitHub Education benefits), then be invited using:

```bash
# Using dfdodzi PAT
DFDODZI_TOKEN="$GH_TOKEN_DFDODZI"
REPO="dfdodzi/project-email-service"

# Invite by username (replace USERNAME)
curl -X PUT \
  -H "Authorization: token $DFDODZI_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/$REPO/collaborators/USERNAME" \
  -d '{"permission":"push"}'

# For dfdnusenu (admin)
curl -X PUT \
  -H "Authorization: token $DFDODZI_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/$REPO/collaborators/dfdnusenu" \
  -d '{"permission":"admin"}'
```

---

## Notes

- All members are students at the University of Ghana (st.ug.edu.gh)
- The `dfdodzi` account is the repository owner
- The `dfdnusenu` account should have admin privileges once created
- The `daptordarattler` account has admin access for CI/CD automation
- PAT for dfdodzi is stored in `~/.zshrc` as `GH_TOKEN_DFDODZI`
- SSH key for dfdodzi is at `~/.ssh/dfdodzi-nusenu` with Host alias `github-dfdodzi` in `~/.ssh/config`
