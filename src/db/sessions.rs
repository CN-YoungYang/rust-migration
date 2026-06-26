use crate::error::Result;
use chrono::{DateTime, Duration, Utc};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, FromRow)]
pub struct DbSession {
    pub id: String,
    pub user_id: String,
    pub csrf_token: String,
    pub expires_at: DateTime<Utc>,
}

pub async fn create_session(
    db: &SqlitePool,
    user_id: &str,
    ttl_secs: u64,
    max_sessions: usize,
) -> Result<DbSession> {
    cleanup_expired_sessions(db).await?;

    let session = DbSession {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: user_id.to_string(),
        csrf_token: uuid::Uuid::new_v4().to_string(),
        expires_at: Utc::now() + Duration::seconds(ttl_secs.min(i64::MAX as u64) as i64),
    };
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO AppSession (id, userId, csrfToken, expiresAt, createdAt, lastSeenAt)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&session.id)
    .bind(&session.user_id)
    .bind(&session.csrf_token)
    .bind(session.expires_at)
    .bind(now)
    .bind(now)
    .execute(db)
    .await?;

    enforce_session_cap(db, max_sessions).await?;
    Ok(session)
}

pub async fn find_session(db: &SqlitePool, id: &str) -> Result<Option<DbSession>> {
    let session = sqlx::query_as::<_, DbSession>(
        "SELECT id, userId as user_id, csrfToken as csrf_token, expiresAt as expires_at
         FROM AppSession WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;

    let Some(session) = session else {
        return Ok(None);
    };
    if Utc::now() >= session.expires_at {
        delete_session(db, id).await?;
        return Ok(None);
    }
    Ok(Some(session))
}

pub async fn delete_session(db: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM AppSession WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn cleanup_expired_sessions(db: &SqlitePool) -> Result<u64> {
    let result = sqlx::query("DELETE FROM AppSession WHERE expiresAt <= ?")
        .bind(Utc::now())
        .execute(db)
        .await?;
    Ok(result.rows_affected())
}

async fn enforce_session_cap(db: &SqlitePool, max_sessions: usize) -> Result<()> {
    if max_sessions == 0 {
        sqlx::query("DELETE FROM AppSession").execute(db).await?;
        return Ok(());
    }

    sqlx::query(
        "DELETE FROM AppSession
         WHERE id NOT IN (
             SELECT id FROM AppSession ORDER BY expiresAt DESC LIMIT ?
         )",
    )
    .bind(max_sessions as i64)
    .execute(db)
    .await?;
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
            "CREATE TABLE AppSession (
                id TEXT PRIMARY KEY,
                userId TEXT NOT NULL,
                csrfToken TEXT NOT NULL,
                expiresAt TEXT NOT NULL,
                createdAt TEXT NOT NULL,
                lastSeenAt TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("session table should be created");
        pool
    }

    #[tokio::test]
    async fn creates_finds_and_deletes_session() {
        let pool = test_pool().await;
        let session = create_session(&pool, "u1", 3600, 100)
            .await
            .expect("session should be created");

        let found = find_session(&pool, &session.id)
            .await
            .expect("session lookup should succeed")
            .expect("session should exist");
        assert_eq!(found.user_id, "u1");
        assert_eq!(found.csrf_token, session.csrf_token);

        delete_session(&pool, &session.id)
            .await
            .expect("session should be deleted");
        assert!(find_session(&pool, &session.id)
            .await
            .expect("lookup should succeed")
            .is_none());
    }

    #[tokio::test]
    async fn removes_expired_sessions() {
        let pool = test_pool().await;
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO AppSession (id, userId, csrfToken, expiresAt, createdAt, lastSeenAt)
             VALUES ('expired', 'u1', 'csrf', ?, ?, ?)",
        )
        .bind(now - Duration::seconds(1))
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("expired session should be inserted");

        let removed = cleanup_expired_sessions(&pool)
            .await
            .expect("cleanup should succeed");
        assert_eq!(removed, 1);
    }
}
