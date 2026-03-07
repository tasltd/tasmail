# RustMail — Project Team Members

**Project:** RustMail Self-Hosted Email Service
**Institution:** University of Ghana
**Updated:** 2026-03-07

---

## Team Roster

| # | Name | Student ID | Email | GitHub Role |
|---|------|-----------|-------|-------------|
| 1 | Dominic Fui Nusenu Dodzi | 22103426 | dfdodzi-nusenu@st.ug.edu.gh | **Owner** (dfdodzi) |
| 2 | Dominic Fui Nusenu Dodzi | — | dfdnusenu@gmail.com | **Admin** (dfdnusenu — pending GitHub account creation) |
| 3 | Benard Obeng Akoto | 22012184 | baobeng019@st.ug.edu.gh | Collaborator |
| 4 | Stephen Nyarko | 22247080 | snyarko055@st.ug.edu.gh | Collaborator |
| 5 | Gabriel Neequaye | 22152238 | gneequaye006@st.ug.edu.gh | Collaborator |
| 6 | Wisdom Sabadu | 22102174 | wsabadu@st.ug.edu.gh | Collaborator |
| 7 | Richmond Sagoe | 22017399 | rsagoe009@st.ug.edu.gh | Collaborator |
| 8 | Christiana Amoak Abisitemi | 22032136 | caamoak@st.ug.edu.gh | Collaborator |

---

## GitHub Accounts

Once each member creates a GitHub account, record their usernames here:

| Name | GitHub Username | Account Confirmed |
|------|----------------|-------------------|
| Dominic Fui Nusenu Dodzi | `dfdodzi` | Yes |
| Dominic Fui Nusenu Dodzi | `dfdnusenu` | Pending — needs GitHub account |
| Benard Obeng Akoto | — | Pending |
| Stephen Nyarko | — | Pending |
| Gabriel Neequaye | — | Pending |
| Wisdom Sabadu | — | Pending |
| Richmond Sagoe | — | Pending |
| Christiana Amoak Abisitemi | — | Pending |

---

## Invite Instructions

After the repo transfer to `dfdodzi` completes, run these commands to add collaborators.

**By GitHub username** (preferred — each member must first create a GitHub account):

```bash
REPO="dfdodzi/project-email-service"

# Add dfdnusenu as admin (once account exists)
gh api repos/$REPO/collaborators/dfdnusenu -X PUT -f permission=admin

# Add team members as collaborators (write access)
gh api repos/$REPO/collaborators/USERNAME -X PUT -f permission=push
```

Replace `USERNAME` with each member's GitHub username once they create accounts.

**Via GitHub web UI** (alternative):

1. Go to https://github.com/dfdodzi/project-email-service/settings/access
2. Click "Add people"
3. Search by GitHub username or invite by email
4. Set permission level (Admin for dfdnusenu, Write for others)

---

## Notes

- All members are students at the University of Ghana (st.ug.edu.gh)
- The `dfdodzi` account is the repository owner
- The `dfdnusenu` account (dfdnusenu@gmail.com) should have admin privileges
- GitHub requires each collaborator to have a GitHub account to accept repository invitations
- Members who don't yet have GitHub accounts should sign up at https://github.com/signup using their `@st.ug.edu.gh` email for GitHub Education benefits
