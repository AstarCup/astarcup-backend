-- 给比赛房间与玩家对战增加赛季字段（幂等，可重复执行）
-- 赛季区分：房间、对战、赛程都挂在同一套赛季 ID（s1 / s2 …）上，
-- 赛程创建时继承所属房间的赛季。

ALTER TABLE "MatchRoom"
    ADD COLUMN IF NOT EXISTS "season" TEXT;

CREATE INDEX IF NOT EXISTS "MatchRoom_season_idx"
    ON "MatchRoom" ("season");

ALTER TABLE "PlayerMatchup"
    ADD COLUMN IF NOT EXISTS "season" TEXT;

CREATE INDEX IF NOT EXISTS "PlayerMatchup_season_idx"
    ON "PlayerMatchup" ("season");

-- 历史数据如需归到当前赛季，可手动执行（默认不自动执行，避免错误标记）：
-- UPDATE "MatchRoom"
-- SET "season" = (SELECT lower(trim(both '"' from "setting_value"))
--                 FROM "TournamentSetting" WHERE "setting_key" = 'current_season')
-- WHERE "season" IS NULL;
-- UPDATE "PlayerMatchup"
-- SET "season" = (SELECT lower(trim(both '"' from "setting_value"))
--                 FROM "TournamentSetting" WHERE "setting_key" = 'current_season')
-- WHERE "season" IS NULL;
