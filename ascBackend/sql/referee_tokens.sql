-- 裁判控制台：工作人员自己的 osu! OAuth token（方案 B：每人绑定自己的账号）
-- 幂等，可重复执行。

CREATE TABLE IF NOT EXISTS referee_tokens (
    osu_id VARCHAR(64) PRIMARY KEY,
    username VARCHAR(128) NOT NULL DEFAULT '',
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at TIMESTAMP NOT NULL,
    scope TEXT NOT NULL DEFAULT '',
    bound_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
