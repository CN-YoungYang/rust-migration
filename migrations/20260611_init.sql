CREATE TABLE IF NOT EXISTS AppUser (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    passwordHash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'USER',
    enabled INTEGER NOT NULL DEFAULT 1,
    note TEXT,
    createdAt TEXT NOT NULL,
    updatedAt TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS CheckinAccount (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    siteType TEXT NOT NULL DEFAULT 'new-api',
    baseUrl TEXT NOT NULL,
    userId TEXT,
    authType TEXT NOT NULL DEFAULT 'access_token',
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
    createdAt TEXT NOT NULL,
    updatedAt TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS CheckinRun (
    id TEXT PRIMARY KEY,
    accountId TEXT NOT NULL,
    status TEXT NOT NULL,
    message TEXT,
    durationMs INTEGER,
    triggeredBy TEXT NOT NULL DEFAULT 'manual',
    rawResponse TEXT,
    createdAt TEXT NOT NULL,
    FOREIGN KEY (accountId) REFERENCES CheckinAccount(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_checkin_run_account_created ON CheckinRun(accountId, createdAt);
CREATE INDEX IF NOT EXISTS idx_checkin_run_account_status_created ON CheckinRun(accountId, status, createdAt);

CREATE TABLE IF NOT EXISTS CheckinSetting (
    id TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 0,
    windowStart TEXT NOT NULL DEFAULT '02:00',
    windowEnd TEXT NOT NULL DEFAULT '05:00',
    retryEnabled INTEGER NOT NULL DEFAULT 1,
    maxAttemptsPerDay INTEGER NOT NULL DEFAULT 3,
    updatedAt TEXT NOT NULL
);

INSERT OR IGNORE INTO CheckinSetting (id, enabled, windowStart, windowEnd, retryEnabled, maxAttemptsPerDay, updatedAt)
VALUES ('global', 0, '02:00', '05:00', 1, 3, datetime('now'));
