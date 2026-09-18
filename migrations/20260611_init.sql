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

-- 服务端异步签到批次。批次范围和结果独立保存，避免 Cloudflare 长请求等待外部站点。
CREATE TABLE IF NOT EXISTS CheckinBatch (
    id TEXT PRIMARY KEY,
    createdBy TEXT NOT NULL,
    triggeredBy TEXT NOT NULL DEFAULT 'manual_batch' CHECK (triggeredBy IN ('manual_batch')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'partial_failed', 'failed')),
    total INTEGER NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0,
    succeeded INTEGER NOT NULL DEFAULT 0,
    alreadyChecked INTEGER NOT NULL DEFAULT 0,
    skipped INTEGER NOT NULL DEFAULT 0,
    failed INTEGER NOT NULL DEFAULT 0,
    idempotencyKey TEXT,
    createdAt TEXT NOT NULL,
    startedAt TEXT,
    finishedAt TEXT,
    FOREIGN KEY (createdBy) REFERENCES AppUser(id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_checkin_batch_creator_idempotency
    ON CheckinBatch(createdBy, idempotencyKey)
    WHERE idempotencyKey IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_checkin_batch_status_created
    ON CheckinBatch(status, createdAt);
CREATE INDEX IF NOT EXISTS idx_checkin_batch_creator_created
    ON CheckinBatch(createdBy, createdAt);

-- 账号名称是范围快照的一部分；不对 accountId 建外键，删除账户后仍可查看批次历史。
CREATE TABLE IF NOT EXISTS CheckinBatchItem (
    batchId TEXT NOT NULL,
    accountId TEXT NOT NULL,
    accountName TEXT NOT NULL,
    position INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'already_checked', 'failed', 'skipped')),
    message TEXT,
    runId TEXT,
    createdAt TEXT NOT NULL,
    updatedAt TEXT NOT NULL,
    PRIMARY KEY (batchId, accountId),
    FOREIGN KEY (batchId) REFERENCES CheckinBatch(id) ON DELETE CASCADE
);
-- 一个账号在同一时间只能属于一个待执行/执行中的批次。
CREATE UNIQUE INDEX IF NOT EXISTS idx_checkin_batch_item_active_account
    ON CheckinBatchItem(accountId)
    WHERE status IN ('pending', 'running');
CREATE INDEX IF NOT EXISTS idx_checkin_batch_item_batch_position
    ON CheckinBatchItem(batchId, position);
CREATE INDEX IF NOT EXISTS idx_checkin_batch_item_account
    ON CheckinBatchItem(accountId, status);

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
    -- 定时调度签到专用的相邻账户随机延迟（秒）；批量手动签到用 batchDelayMin/Max。
    scheduledDelayMin INTEGER NOT NULL DEFAULT 3,
    scheduledDelayMax INTEGER NOT NULL DEFAULT 10,
    cleanupKeepLatest INTEGER NOT NULL DEFAULT 500,
    -- 调度触发计划：标准 5 段 cron 表达式的 JSON 数组，命中任一即触发一轮签到。
    -- v2.6.0 起替代 windowStart/windowEnd（列保留但不再读写）。
    scheduleCron TEXT NOT NULL DEFAULT '["*/5 2-5 * * *"]',
    updatedAt TEXT NOT NULL
);

-- 注意：此处不写入 batchDelayMin / batchDelayMax / scheduledDelayMin / scheduledDelayMax / scheduleCron。
-- batchDelayMin/Max 在 v2.2.2 才引入，scheduledDelayMin/Max 在本次拆分才引入，scheduleCron 在 v2.6.0 引入。
-- 旧库的 CheckinSetting 可能尚未包含这些列（CREATE TABLE IF NOT EXISTS 不会给老表补列）。
-- 在此 INSERT 引用缺失列会导致启动报错：
--   "table CheckinSetting has no column named batchDelayMin"
-- 新库通过上面的列定义 DEFAULT 取得 3 / 10 / 默认 cron；
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
