use crate::error::Result;
use crate::models::AppUser;
use chrono::Utc;
use sqlx::SqlitePool;

/// Find user by username (includes password hash for authentication)
pub async fn find_user_by_username(db: &SqlitePool, username: &str) -> Result<Option<AppUser>> {
    let user = sqlx::query_as::<_, AppUser>(
        "SELECT id, username, passwordHash, role, enabled, note, createdAt, updatedAt \
         FROM AppUser WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(db)
    .await?;
    Ok(user)
}

/// Find user by ID (includes password hash for update operations)
pub async fn find_user_by_id(db: &SqlitePool, id: &str) -> Result<Option<AppUser>> {
    let user = sqlx::query_as::<_, AppUser>(
        "SELECT id, username, passwordHash, role, enabled, note, createdAt, updatedAt \
         FROM AppUser WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(user)
}

/// List all users (excludes password hash for security)
pub async fn list_users(db: &SqlitePool) -> Result<Vec<AppUser>> {
    let users = sqlx::query_as::<_, AppUser>(
        "SELECT id, username, '' as passwordHash, role, enabled, note, createdAt, updatedAt \
         FROM AppUser ORDER BY createdAt DESC",
    )
    .fetch_all(db)
    .await?;
    Ok(users)
}

/// Lightweight query: returns id -> username mapping, avoiding pulling passwordHash and other sensitive fields
pub async fn list_user_id_name_map(
    db: &SqlitePool,
) -> Result<std::collections::HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, username FROM AppUser")
        .fetch_all(db)
        .await?;
    Ok(rows.into_iter().collect())
}

/// Create a new user
pub async fn create_user(
    db: &SqlitePool,
    username: &str,
    password_hash: &str,
    role: &str,
    enabled: bool,
    note: Option<&str>,
) -> Result<AppUser> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let user = sqlx::query_as::<_, AppUser>(
        "INSERT INTO AppUser (id, username, passwordHash, role, enabled, note, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"
    )
    .bind(&id)
    .bind(username)
    .bind(password_hash)
    .bind(role)
    .bind(enabled)
    .bind(note)
    .bind(now)
    .bind(now)
    .fetch_one(db)
    .await?;

    Ok(user)
}

/// Update user details
pub async fn update_user(
    db: &SqlitePool,
    id: &str,
    username: Option<&str>,
    password_hash: Option<&str>,
    role: Option<&str>,
    enabled: Option<bool>,
    note: Option<&str>,
) -> Result<()> {
    let now = Utc::now();
    let current = find_user_by_id(db, id)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;

    sqlx::query(
        "UPDATE AppUser SET username = ?, passwordHash = ?, role = ?, enabled = ?, note = ?, updatedAt = ? WHERE id = ?"
    )
    .bind(username.unwrap_or(&current.username))
    .bind(password_hash.unwrap_or(&current.password_hash))
    .bind(role.unwrap_or(&current.role))
    .bind(enabled.unwrap_or(current.enabled))
    .bind(note.or(current.note.as_deref()))
    .bind(now)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

/// Delete user with cascade (accounts and runs)
pub async fn delete_user(db: &SqlitePool, id: &str) -> Result<()> {
    let mut tx = db.begin().await?;

    sqlx::query("DELETE FROM AppSession WHERE userId = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // Cascade: delete runs for accounts owned by this user
    sqlx::query(
        "DELETE FROM CheckinRun WHERE accountId IN (SELECT id FROM CheckinAccount WHERE ownerId = ?)"
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    // Cascade: delete accounts owned by this user
    sqlx::query("DELETE FROM CheckinAccount WHERE ownerId = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // Delete the user
    sqlx::query("DELETE FROM AppUser WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
