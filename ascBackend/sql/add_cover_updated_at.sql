-- 给用户增加 cover 刷新时间戳（幂等，可重复执行）
-- 用于后台任务按「cover 最久未刷新」轮转拉取 osu! 最新封面：
-- 自定义封面（custom_url）会随时变化，仅靠登录时写入会过期。

ALTER TABLE "User"
    ADD COLUMN IF NOT EXISTS "cover_updated_at" TIMESTAMP(3);

CREATE INDEX IF NOT EXISTS "User_cover_updated_at_idx"
    ON "User" ("cover_updated_at");

-- 既有用户先按最后更新时间初始化，保证轮转公平
UPDATE "User"
SET "cover_updated_at" = "updatedAt"
WHERE "cover_updated_at" IS NULL;
