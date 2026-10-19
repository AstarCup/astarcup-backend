-- 给比赛赛程/比分增加赛季字段（幂等，可重复执行）
-- 新赛程创建时会自动写入 current_season；历史数据可手动回填。

ALTER TABLE "MatchSchedule"
    ADD COLUMN IF NOT EXISTS "season" TEXT;

CREATE INDEX IF NOT EXISTS "MatchSchedule_season_idx"
    ON "MatchSchedule" ("season");

-- 如需把历史数据统一归到当前赛季，可手动执行（默认不自动执行，避免错误标注）：
-- UPDATE "MatchSchedule"
-- SET "season" = (
--     SELECT lower(trim(both '"' from "setting_value")) FROM "TournamentSetting" WHERE "setting_key" = 'current_season'
-- )
-- WHERE "season" IS NULL;
