CREATE TABLE IF NOT EXISTS AppUser (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    passwordHash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'USER' CHECK (role IN ('USER', 'ADMIN', 'SUPER_ADMIN')),
    enabled INTEGER NOT NULL DEFAULT 1,
    note TEXT,
    createdAt TEXT NOT NULL,
    updatedAt TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS CheckinAccount (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    siteType TEXT NOT NULL DEFAULT 'new-api' CHECK (siteType IN ('new-api', 'anyrouter', 'x666')),
    baseUrl TEXT NOT NULL,
    userId TEXT,
    ownerId TEXT,
    authType TEXT NOT NULL DEFAULT 'access_token' CHECK (authType IN ('access_token', 'cookie')),
    accessTokenEnc TEXT,
    cookieEnc TEXT,
    customCheckinUrl TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    retryEnabled INTEGER NOT NULL DEFAULT 1,
    lastBalance REAL,
    lastBalanceAt TEXT,
    lastStatus TEXT,
    lastMessage TEXT,
    lastRunAt TEXT,
    note TEXT,
    createdAt TEXT NOT NULL,
    updatedAt TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS CheckinRun (
    id TEXT PRIMARY KEY,
    accountId TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('success', 'failed', 'already_checked', 'pending')),
    message TEXT,
    durationMs INTEGER,
    triggeredBy TEXT NOT NULL DEFAULT 'manual' CHECK (triggeredBy IN ('manual', 'manual_batch', 'scheduled')),
    rawResponse TEXT,
    createdAt TEXT NOT NULL,
    FOREIGN KEY (accountId) REFERENCES CheckinAccount(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_checkin_run_account_created ON CheckinRun(accountId, createdAt);
CREATE INDEX IF NOT EXISTS idx_checkin_run_account_status_created ON CheckinRun(accountId, status, createdAt);
CREATE INDEX IF NOT EXISTS idx_checkin_run_created ON CheckinRun(createdAt);
CREATE INDEX IF NOT EXISTS idx_checkin_account_owner ON CheckinAccount(ownerId);
CREATE INDEX IF NOT EXISTS idx_checkin_account_enabled ON CheckinAccount(enabled);

CREATE TABLE IF NOT EXISTS AppSession (
    id TEXT PRIMARY KEY,
    userId TEXT NOT NULL,
    csrfToken TEXT NOT NULL,
    expiresAt TEXT NOT NULL,
    createdAt TEXT NOT NULL,
    lastSeenAt TEXT NOT NULL,
    FOREIGN KEY (userId) REFERENCES AppUser(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_app_session_user ON AppSession(userId);
CREATE INDEX IF NOT EXISTS idx_app_session_expires ON AppSession(expiresAt);

CREATE TABLE IF NOT EXISTS CheckinSetting (
    id TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 0,
    windowStart TEXT NOT NULL DEFAULT '02:00',
    windowEnd TEXT NOT NULL DEFAULT '05:00',
    retryEnabled INTEGER NOT NULL DEFAULT 1,
    maxAttemptsPerDay INTEGER NOT NULL DEFAULT 3,
    batchDelayMin INTEGER NOT NULL DEFAULT 3,
    batchDelayMax INTEGER NOT NULL DEFAULT 10,
    cleanupKeepLatest INTEGER NOT NULL DEFAULT 500,
    updatedAt TEXT NOT NULL
);

-- 注意：此处不写入 batchDelayMin / batchDelayMax。这两列在 v2.2.2 才引入，
-- 旧库的 CheckinSetting 可能尚未包含（CREATE TABLE IF NOT EXISTS 不会给老表补列）。
-- 在此 INSERT 引用缺失列会导致启动报错：
--   "table CheckinSetting has no column named batchDelayMin"
-- 新库通过上面的列定义 DEFAULT 取得 3 / 10；
-- 旧库由 db::ensure_setting_columns() 运行时补列并修正默认值。
INSERT OR IGNORE INTO CheckinSetting (id, enabled, windowStart, windowEnd, retryEnabled, maxAttemptsPerDay, updatedAt)
VALUES ('global', 0, '02:00', '05:00', 1, 3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));

-- 通知配置表
CREATE TABLE IF NOT EXISTS NotificationConfig (
    id TEXT PRIMARY KEY,
    ownerId TEXT NOT NULL,
    notifyType TEXT NOT NULL CHECK (notifyType IN ('email', 'webhook', 'telegram')),
    enabled INTEGER NOT NULL DEFAULT 1,

    -- 触发条件
    onFailure INTEGER NOT NULL DEFAULT 1,           -- 签到失败时通知
    failureThreshold INTEGER NOT NULL DEFAULT 1,    -- 连续失败 N 次后才通知
    onBalanceLow INTEGER NOT NULL DEFAULT 0,        -- 余额过低时通知
    balanceThreshold REAL,                          -- 余额阈值（美元）

    -- 邮件配置（notifyType = 'email'）
    emailSmtpHost TEXT,
    emailSmtpPort INTEGER,
    emailSmtpUser TEXT,
    emailSmtpPassword TEXT,      -- 加密存储
    emailFrom TEXT,
    emailTo TEXT,                -- 接收邮箱，多个用逗号分隔

    -- Webhook 配置（notifyType = 'webhook'）
    webhookUrl TEXT,
    webhookMethod TEXT DEFAULT 'POST',
    webhookHeaders TEXT,         -- JSON 格式存储自定义 headers

    -- Telegram 配置（notifyType = 'telegram'）
    telegramBotToken TEXT,       -- 加密存储
    telegramChatId TEXT,

    note TEXT,
    createdAt TEXT NOT NULL,
    updatedAt TEXT NOT NULL,

    FOREIGN KEY (ownerId) REFERENCES AppUser(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_notification_owner ON NotificationConfig(ownerId);
CREATE INDEX IF NOT EXISTS idx_notification_enabled ON NotificationConfig(enabled);

-- 失败计数跟踪表（用于计算连续失败次数）
CREATE TABLE IF NOT EXISTS FailureCounter (
    accountId TEXT PRIMARY KEY,
    consecutiveFailures INTEGER NOT NULL DEFAULT 0,
    lastFailedAt TEXT,
    lastNotifiedAt TEXT,
    updatedAt TEXT NOT NULL,
    FOREIGN KEY (accountId) REFERENCES CheckinAccount(id) ON DELETE CASCADE
);
