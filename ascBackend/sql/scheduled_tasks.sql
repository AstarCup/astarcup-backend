-- 计划任务配置表（幂等，可重复执行）
-- 后端调度器按此表配置周期性执行内置任务（src/scheduler.rs），
-- 管理后台可配置启停、节奏与参数（/api/admin/scheduled-tasks）。

CREATE TABLE IF NOT EXISTS scheduled_tasks (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL DEFAULT '',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    interval_secs INTEGER NOT NULL DEFAULT 60,
    start_time TEXT NOT NULL DEFAULT '00:00',
    run_amount INTEGER NOT NULL DEFAULT 1,
    params JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_run_at TIMESTAMP,
    last_run_note TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 默认任务：用户 cover 刷新（每天 0 点起，每分钟 1 个用户；未满 24h 不重复刷新）
INSERT INTO scheduled_tasks (name, title, interval_secs, start_time, run_amount, params)
VALUES ('cover_refresh', '用户 cover 刷新', 60, '00:00', 1, '{"stale_hours": 24}'::jsonb)
ON CONFLICT (name) DO NOTHING;
