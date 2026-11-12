-- 首页画布贴纸（带缩放 / 旋转）
-- 幂等：可重复执行。后端启动时也会自动建表（src/db.rs 的 ensure_schema）。

CREATE TABLE IF NOT EXISTS canvas_stickers (
    id SERIAL PRIMARY KEY,
    page VARCHAR(32) NOT NULL DEFAULT 'home',
    token VARCHAR(128) NOT NULL,
    x DOUBLE PRECISION NOT NULL,
    y DOUBLE PRECISION NOT NULL,
    scale DOUBLE PRECISION NOT NULL DEFAULT 1,
    rotation DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_by VARCHAR(64) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS canvas_stickers_page_idx
    ON canvas_stickers (page);
CREATE INDEX IF NOT EXISTS canvas_stickers_rate_idx
    ON canvas_stickers (created_by, created_at);
