use crate::error::Result;
use crate::models::AppUser;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct UserAccountStats {
    pub account_count: i64,
    pub enabled_account_count: i64,
    pub failed_account_count: i64,
    pub last_run_at: Option<DateTime<Utc>>,
}

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

pub async fn find_username_by_id(db: &SqlitePool, id: &str) -> Result<Option<String>> {
    let username = sqlx::query_scalar::<_, String>("SELECT username FROM AppUser WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?;
    Ok(username)
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

pub async fn list_user_account_stats(
    db: &SqlitePool,
) -> Result<std::collections::HashMap<String, UserAccountStats>> {
    let rows = sqlx::query_as::<_, (String, i64, Option<i64>, Option<i64>, Option<DateTime<Utc>>)>(
        "SELECT
            u.id,
            COUNT(a.id) as accountCount,
            SUM(CASE WHEN a.enabled = 1 THEN 1 ELSE 0 END) as enabledAccountCount,
            SUM(CASE WHEN a.lastStatus = 'failed' THEN 1 ELSE 0 END) as failedAccountCount,
            MAX(a.lastRunAt) as lastRunAt
         FROM AppUser u
         LEFT JOIN CheckinAccount a ON a.ownerId = u.id
         GROUP BY u.id",
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(user_id, account_count, enabled_account_count, failed_account_count, last_run_at)| {
                (
                    user_id.clone(),
                    UserAccountStats {
                        account_count,
                        enabled_account_count: enabled_account_count.unwrap_or(0),
                        failed_account_count: failed_account_count.unwrap_or(0),
                        last_run_at,
                    },
                )
            },
        )
        .collect())
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");

        sqlx::query(
            "CREATE TABLE AppUser (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                passwordHash TEXT NOT NULL,
                role TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                note TEXT,
                createdAt TEXT NOT NULL,
                updatedAt TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("user table should be created");

        sqlx::query(
            "CREATE TABLE CheckinAccount (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                siteType TEXT NOT NULL,
                baseUrl TEXT NOT NULL,
                ownerId TEXT,
                authType TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                retryEnabled INTEGER NOT NULL,
                lastStatus TEXT,
                lastRunAt TEXT,
                createdAt TEXT NOT NULL,
                updatedAt TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("account table should be created");

        pool
    }

    async fn insert_user(pool: &SqlitePool, id: &str) {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO AppUser (id, username, passwordHash, role, enabled, createdAt, updatedAt)
             VALUES (?, ?, 'hash', 'USER', 1, ?, ?)",
        )
        .bind(id)
        .bind(format!("user-{id}"))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("user should be inserted");
    }

    async fn insert_account(
        pool: &SqlitePool,
        id: &str,
        owner_id: &str,
        enabled: bool,
        last_status: Option<&str>,
        last_run_at: Option<DateTime<Utc>>,
    ) {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO CheckinAccount (
                id, name, siteType, baseUrl, ownerId, authType,
                enabled, retryEnabled, lastStatus, lastRunAt, createdAt, updatedAt
             ) VALUES (?, ?, 'new-api', 'https://example.com', ?, 'access_token', ?, 1, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(format!("account-{id}"))
        .bind(owner_id)
        .bind(enabled)
        .bind(last_status)
        .bind(last_run_at)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("account should be inserted");
    }

    #[tokio::test]
    async fn list_user_account_stats_aggregates_counts_and_last_run() {
        let pool = test_pool().await;
        insert_user(&pool, "user-a").await;
        insert_user(&pool, "user-b").await;
        let older = Utc::now() - chrono::Duration::hours(2);
        let newer = Utc::now();

        insert_account(&pool, "a-1", "user-a", true, Some("success"), Some(older)).await;
        insert_account(&pool, "a-2", "user-a", false, Some("failed"), Some(newer)).await;
        insert_account(&pool, "b-1", "user-b", true, None, None).await;

        let stats = list_user_account_stats(&pool)
            .await
            .expect("user stats should load");

        let user_a = stats.get("user-a").expect("user-a stats should exist");
        assert_eq!(user_a.account_count, 2);
        assert_eq!(user_a.enabled_account_count, 1);
        assert_eq!(user_a.failed_account_count, 1);
        assert_eq!(user_a.last_run_at, Some(newer));

        let user_b = stats.get("user-b").expect("user-b stats should exist");
        assert_eq!(user_b.account_count, 1);
        assert_eq!(user_b.enabled_account_count, 1);
        assert_eq!(user_b.failed_account_count, 0);
        assert_eq!(user_b.last_run_at, None);
    }
}
