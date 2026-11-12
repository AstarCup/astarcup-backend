-- 选手卡片表情贴纸（/registrations）
-- 幂等：可重复执行。后端启动时也会自动建表（src/db.rs 的 ensure_schema）。

CREATE TABLE IF NOT EXISTS player_stickers (
    id SERIAL PRIMARY KEY,
    osu_id VARCHAR(64) NOT NULL,
    token VARCHAR(128) NOT NULL,
    x DOUBLE PRECISION NOT NULL,
    y DOUBLE PRECISION NOT NULL,
    created_by VARCHAR(64) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS player_stickers_osu_idx
    ON player_stickers (osu_id);
CREATE INDEX IF NOT EXISTS player_stickers_created_by_idx
    ON player_stickers (created_by);

-- 限速查询用：按人 + 时间
CREATE INDEX IF NOT EXISTS player_stickers_rate_idx
    ON player_stickers (created_by, created_at);
