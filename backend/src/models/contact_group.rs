// Added: Contact group model for organizing contacts into labeled groups (TMAIL-119)
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ContactGroup {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub color: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateContactGroup {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContactGroup {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ContactGroupMember {
    pub contact_group_id: Uuid,
    pub contact_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub contact_id: Uuid,
}

impl ContactGroup {
    // PURPOSE: List all contact groups owned by a user
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<ContactGroup>, sqlx::Error> {
        sqlx::query_as::<_, ContactGroup>(
            "SELECT * FROM contact_groups WHERE user_id = $1 ORDER BY name ASC"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
    }

    // PURPOSE: Find a single contact group by id and user
    pub async fn find_by_id(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<Option<ContactGroup>, sqlx::Error> {
        sqlx::query_as::<_, ContactGroup>(
            "SELECT * FROM contact_groups WHERE id = $1 AND user_id = $2"
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
    }

    // PURPOSE: Create a new contact group
    pub async fn create(pool: &PgPool, user_id: Uuid, input: &CreateContactGroup) -> Result<ContactGroup, sqlx::Error> {
        sqlx::query_as::<_, ContactGroup>(
            "INSERT INTO contact_groups (user_id, name, color) VALUES ($1, $2, $3) RETURNING *"
        )
        .bind(user_id)
        .bind(&input.name)
        .bind(&input.color)
        .fetch_one(pool)
        .await
    }

    // PURPOSE: Update an existing contact group
    pub async fn update(pool: &PgPool, id: Uuid, user_id: Uuid, input: &UpdateContactGroup) -> Result<Option<ContactGroup>, sqlx::Error> {
        sqlx::query_as::<_, ContactGroup>(
            "UPDATE contact_groups SET
                name = COALESCE($3, name),
                color = COALESCE($4, color)
            WHERE id = $1 AND user_id = $2 RETURNING *"
        )
        .bind(id)
        .bind(user_id)
        .bind(&input.name)
        .bind(&input.color)
        .fetch_optional(pool)
        .await
    }

    // PURPOSE: Delete a contact group (cascade removes members)
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM contact_groups WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl ContactGroupMember {
    // PURPOSE: Add a contact to a group
    pub async fn add_to_group(pool: &PgPool, group_id: Uuid, contact_id: Uuid) -> Result<ContactGroupMember, sqlx::Error> {
        sqlx::query_as::<_, ContactGroupMember>(
            "INSERT INTO contact_group_members (contact_group_id, contact_id) VALUES ($1, $2) RETURNING *"
        )
        .bind(group_id)
        .bind(contact_id)
        .fetch_one(pool)
        .await
    }

    // PURPOSE: Remove a contact from a group
    pub async fn remove_from_group(pool: &PgPool, group_id: Uuid, contact_id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM contact_group_members WHERE contact_group_id = $1 AND contact_id = $2"
        )
        .bind(group_id)
        .bind(contact_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // PURPOSE: List all contact IDs in a group (used to join with contacts)
    pub async fn list_contact_ids_in_group(pool: &PgPool, group_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
        let rows = sqlx::query_scalar::<_, Uuid>(
            "SELECT contact_id FROM contact_group_members WHERE contact_group_id = $1"
        )
        .bind(group_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contact_group_serialization() {
        let id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let group = ContactGroup {
            id,
            user_id,
            name: "Work".to_string(),
            color: Some("#ff0000".to_string()),
            created_at: now,
        };

        let json = serde_json::to_value(&group).unwrap();
        assert_eq!(json["name"], "Work");
        assert_eq!(json["color"], "#ff0000");
        assert_eq!(json["id"], id.to_string());
        assert_eq!(json["user_id"], user_id.to_string());
    }

    #[test]
    fn test_contact_group_serialization_no_color() {
        let group = ContactGroup {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "Friends".to_string(),
            color: None,
            created_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(&group).unwrap();
        assert_eq!(json["name"], "Friends");
        assert!(json["color"].is_null());
    }

    #[test]
    fn test_create_contact_group_deserialization() {
        let json = serde_json::json!({
            "name": "Family",
            "color": "#00ff00"
        });

        let create: CreateContactGroup = serde_json::from_value(json).unwrap();
        assert_eq!(create.name, "Family");
        assert_eq!(create.color.unwrap(), "#00ff00");
    }

    #[test]
    fn test_create_contact_group_minimal() {
        let json = serde_json::json!({
            "name": "Misc"
        });

        let create: CreateContactGroup = serde_json::from_value(json).unwrap();
        assert_eq!(create.name, "Misc");
        assert!(create.color.is_none());
    }

    #[test]
    fn test_create_contact_group_missing_name_fails() {
        let json = serde_json::json!({
            "color": "#aabbcc"
        });

        let result = serde_json::from_value::<CreateContactGroup>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_contact_group_partial() {
        let json = serde_json::json!({
            "name": "Renamed"
        });

        let update: UpdateContactGroup = serde_json::from_value(json).unwrap();
        assert_eq!(update.name.unwrap(), "Renamed");
        assert!(update.color.is_none());
    }

    #[test]
    fn test_update_contact_group_empty() {
        let json = serde_json::json!({});

        let update: UpdateContactGroup = serde_json::from_value(json).unwrap();
        assert!(update.name.is_none());
        assert!(update.color.is_none());
    }

    #[test]
    fn test_add_member_request_deserialization() {
        let contact_id = Uuid::new_v4();
        let json = serde_json::json!({
            "contact_id": contact_id.to_string()
        });

        let req: AddMemberRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.contact_id, contact_id);
    }

    #[test]
    fn test_contact_group_member_serialization() {
        let group_id = Uuid::new_v4();
        let contact_id = Uuid::new_v4();

        let member = ContactGroupMember {
            contact_group_id: group_id,
            contact_id,
        };

        let json = serde_json::to_value(&member).unwrap();
        assert_eq!(json["contact_group_id"], group_id.to_string());
        assert_eq!(json["contact_id"], contact_id.to_string());
    }

    #[test]
    fn test_update_contact_group_all_fields() {
        let json = serde_json::json!({
            "name": "Updated Group",
            "color": "#112233"
        });

        let update: UpdateContactGroup = serde_json::from_value(json).unwrap();
        assert_eq!(update.name.unwrap(), "Updated Group");
        assert_eq!(update.color.unwrap(), "#112233");
    }
}
