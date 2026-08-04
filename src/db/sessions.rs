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

/// 查找会话并按需滑动续期。命中且未过期时，若已超过 TTL 的一半则更新
/// `lastSeenAt` 并把 `expiresAt` 顺延一个 TTL；否则直接返回，避免每请求写库。
pub async fn find_session_and_renew(
    db: &SqlitePool,
    id: &str,
    ttl_secs: i64,
) -> Result<Option<DbSession>> {
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
    let now = Utc::now();
    if now >= session.expires_at {
        delete_session(db, id).await?;
        return Ok(None);
    }

    // 已超过 TTL 的一半才续期，控制写库频率
    if (session.expires_at - now).num_seconds() <= ttl_secs / 2 {
        let new_expires = now + Duration::seconds(ttl_secs);
        sqlx::query("UPDATE AppSession SET lastSeenAt = ?, expiresAt = ? WHERE id = ?")
            .bind(now)
            .bind(new_expires)
            .bind(id)
            .execute(db)
            .await?;
        return Ok(Some(DbSession {
            id: session.id,
            user_id: session.user_id,
            csrf_token: session.csrf_token,
            expires_at: new_expires,
        }));
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

/// 删除某用户的全部会话。改密后调用，使旧会话失效（防止旧 Cookie 继续有效）。
pub async fn delete_sessions_for_user(db: &SqlitePool, user_id: &str) -> Result<u64> {
    let result = sqlx::query("DELETE FROM AppSession WHERE userId = ?")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(result.rows_affected())
}

/// 清理过期会话。`lastSeenAt` 距今不足 1 分钟的会话被排除，避免与
/// `find_session_and_renew` 的续期竞态：清理 DELETE 基于快照的旧 `expiresAt`
/// 可能把刚被续期（expiresAt 已顺延、lastSeenAt 刚更新）的会话删掉（L3）。
/// 这类刚活跃过的会话即使恰好过期也会在下一次清理时再被评估，至多延迟 5 分钟。
pub async fn cleanup_expired_sessions(db: &SqlitePool) -> Result<u64> {
    let now = Utc::now();
    let result = sqlx::query(
        "DELETE FROM AppSession
         WHERE expiresAt <= ?
           AND lastSeenAt <= ?",
    )
    .bind(now)
    .bind(now - Duration::seconds(60))
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
        // TTL 设为很大，本次查找不会触发续期，保持查找原值语义
        let ttl: i64 = 3600;
        let session = create_session(&pool, "u1", ttl as u64, 100)
            .await
            .expect("session should be created");

        let found = find_session_and_renew(&pool, &session.id, ttl)
            .await
            .expect("session lookup should succeed")
            .expect("session should exist");
        assert_eq!(found.user_id, "u1");
        assert_eq!(found.csrf_token, session.csrf_token);

        delete_session(&pool, &session.id)
            .await
            .expect("session should be deleted");
        assert!(find_session_and_renew(&pool, &session.id, ttl)
            .await
            .expect("lookup should succeed")
            .is_none());
    }

    #[tokio::test]
    async fn removes_expired_sessions_but_keeps_recently_renewed() {
        let pool = test_pool().await;
        let now = Utc::now();
        // 已过期且长期未使用（lastSeenAt 超过 1 分钟）：应被清理
        sqlx::query(
            "INSERT INTO AppSession (id, userId, csrfToken, expiresAt, createdAt, lastSeenAt)
             VALUES ('expired', 'u1', 'csrf', ?, ?, ?)",
        )
        .bind(now - Duration::seconds(1))
        .bind(now - Duration::seconds(3600))
        .bind(now - Duration::seconds(61))
        .execute(&pool)
        .await
        .expect("expired session should be inserted");

        // 已过期但 1 分钟内刚续期（lastSeenAt 接近 now）：应被排除，避免与续期竞态
        sqlx::query(
            "INSERT INTO AppSession (id, userId, csrfToken, expiresAt, createdAt, lastSeenAt)
             VALUES ('recent-renew', 'u1', 'csrf', ?, ?, ?)",
        )
        .bind(now - Duration::seconds(1))
        .bind(now - Duration::seconds(3600))
        .bind(now)
        .execute(&pool)
        .await
        .expect("recently renewed session should be inserted");

        let removed = cleanup_expired_sessions(&pool)
            .await
            .expect("cleanup should succeed");
        assert_eq!(removed, 1);

        let (remaining,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM AppSession WHERE id = 'recent-renew'")
                .fetch_one(&pool)
                .await
                .expect("count should succeed");
        assert_eq!(remaining, 1);
    }

    #[tokio::test]
    async fn deletes_all_sessions_for_user() {
        let pool = test_pool().await;
        let ttl: u64 = 3600;
        let s1 = create_session(&pool, "u1", ttl, 100).await.unwrap();
        let s2 = create_session(&pool, "u1", ttl, 100).await.unwrap();
        let s3 = create_session(&pool, "u2", ttl, 100).await.unwrap();

        let removed = delete_sessions_for_user(&pool, "u1").await.unwrap();
        assert_eq!(removed, 2);
        assert!(find_session_and_renew(&pool, &s1.id, ttl as i64)
            .await
            .unwrap()
            .is_none());
        assert!(find_session_and_renew(&pool, &s2.id, ttl as i64)
            .await
            .unwrap()
            .is_none());
        // 其他用户会话不受影响
        assert!(find_session_and_renew(&pool, &s3.id, ttl as i64)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn renew_extends_expires_only_after_half_ttl() {
        let pool = test_pool().await;
        let ttl: i64 = 3600;
        let session = create_session(&pool, "u1", ttl as u64, 100)
            .await
            .expect("session should be created");

        // 刚创建，距过期还远，不应续期
        let found = find_session_and_renew(&pool, &session.id, ttl)
            .await
            .expect("lookup should succeed")
            .expect("session should exist");
        assert_eq!(found.expires_at, session.expires_at);

        // 把 expiresAt 提前到只剩 < ttl/2，再查应被续期
        let near_expiry = Utc::now() + Duration::seconds(ttl / 4);
        sqlx::query("UPDATE AppSession SET expiresAt = ? WHERE id = ?")
            .bind(near_expiry)
            .bind(&session.id)
            .execute(&pool)
            .await
            .expect("should update");

        let before = Utc::now();
        let renewed = find_session_and_renew(&pool, &session.id, ttl)
            .await
            .expect("lookup should succeed")
            .expect("session should exist");
        assert!(renewed.expires_at > near_expiry);
        assert!(renewed.expires_at >= before + Duration::seconds(ttl - 5));
    }
}
