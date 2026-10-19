-- Bot 平台身份绑定（幂等，可重复执行）
-- 作用：把 AstrBot 的平台身份（QQ 号 / QQ openid 等）绑定到 osu 账号。
-- 绑定必须由用户在网站登录后生成一次性绑定码，不能仅凭 QQ 号认人。

CREATE TABLE IF NOT EXISTS bot_platform_bindings (
    id SERIAL PRIMARY KEY,
    platform VARCHAR(64) NOT NULL,
    platform_user_id VARCHAR(128) NOT NULL,
    osu_id VARCHAR(64) NOT NULL,
    display_name VARCHAR(128),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS bot_platform_bindings_platform_user_idx
    ON bot_platform_bindings (platform, platform_user_id);
CREATE UNIQUE INDEX IF NOT EXISTS bot_platform_bindings_platform_osu_idx
    ON bot_platform_bindings (platform, osu_id);
CREATE INDEX IF NOT EXISTS bot_platform_bindings_osu_idx
    ON bot_platform_bindings (osu_id);

CREATE TABLE IF NOT EXISTS bot_bind_codes (
    id SERIAL PRIMARY KEY,
    code_hash VARCHAR(64) NOT NULL UNIQUE,
    osu_id VARCHAR(64) NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    used_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS bot_bind_codes_osu_idx
    ON bot_bind_codes (osu_id);

-- 私聊一键绑定链接（GET 只查看，POST 才消耗）
CREATE TABLE IF NOT EXISTS bot_bind_links (
    id SERIAL PRIMARY KEY,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    platform VARCHAR(64) NOT NULL,
    platform_user_id VARCHAR(128) NOT NULL,
    display_name VARCHAR(128),
    expires_at TIMESTAMP NOT NULL,
    used_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS bot_bind_links_identity_idx
    ON bot_bind_links (platform, platform_user_id);
