-- Astra Cup dev 数据库初始化脚本（与线上 Prisma schema 一致）
-- 用法：psql -h localhost -p 5433 -U ascDev -d ascDev -f sql/dev_schema.sql

-- ============ 枚举类型 ============
CREATE TYPE "UserGroup" AS ENUM ('player', 'admin');
CREATE TYPE "RegistrationStatus" AS ENUM ('not_registered', 'registered', 'approved');
CREATE TYPE "RoomStatus" AS ENUM ('open', 'full', 'closed');
CREATE TYPE "ScheduleStatus" AS ENUM ('pending', 'confirmed', 'completed', 'cancelled');
CREATE TYPE "MatchupStatus" AS ENUM ('available', 'pending', 'scheduled', 'completed');
CREATE TYPE "MessageType" AS ENUM ('match_invitation', 'system_notification', 'admin_message');
CREATE TYPE "MessageStatus" AS ENUM ('unread', 'read', 'archived');
CREATE TYPE "StaffRole" AS ENUM ('referee', 'commentator', 'streamer');

-- ============ User ============
CREATE TABLE "User" (
    "id" SERIAL PRIMARY KEY,
    "osuId" TEXT NOT NULL UNIQUE,
    "username" TEXT NOT NULL,
    "registeredAt" TIMESTAMP(3) NOT NULL,
    "avatar_url" TEXT,
    "pp" DOUBLE PRECISION,
    "global_rank" INTEGER,
    "country_rank" INTEGER,
    "country" TEXT NOT NULL,
    "userGroup" "UserGroup" NOT NULL DEFAULT 'player',
    "registrationStatus" "RegistrationStatus" NOT NULL DEFAULT 'not_registered',
    "season" TEXT[],
    "accuracy" DOUBLE PRECISION,
    "stamina" DOUBLE PRECISION,
    "firstSight" DOUBLE PRECISION,
    "strategy" DOUBLE PRECISION,
    "experience" DOUBLE PRECISION,
    "customKey" TEXT,
    "customValue" DOUBLE PRECISION,
    "cover_custom_url" TEXT,
    "cover_url" TEXT,
    "cover_id" TEXT,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL
);
CREATE INDEX "User_osuId_idx" ON "User"("osuId");
CREATE INDEX "User_username_idx" ON "User"("username");
CREATE INDEX "User_userGroup_idx" ON "User"("userGroup");
CREATE INDEX "User_registrationStatus_idx" ON "User"("registrationStatus");
CREATE INDEX "User_season_idx" ON "User"("season");

-- ============ MatchRoom ============
CREATE TABLE "MatchRoom" (
    "id" SERIAL PRIMARY KEY,
    "room_name" TEXT NOT NULL,
    "round_number" INTEGER NOT NULL,
    "match_date" TIMESTAMP(3) NOT NULL,
    "match_time" TIMESTAMP(3) NOT NULL,
    "match_number" INTEGER NOT NULL,
    "max_participants" INTEGER NOT NULL DEFAULT 2,
    "status" "RoomStatus" NOT NULL DEFAULT 'open',
    "description" TEXT,
    "created_by" TEXT NOT NULL,
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(3) NOT NULL
);
CREATE INDEX "MatchRoom_round_number_idx" ON "MatchRoom"("round_number");
CREATE INDEX "MatchRoom_match_date_idx" ON "MatchRoom"("match_date");
CREATE INDEX "MatchRoom_status_idx" ON "MatchRoom"("status");
CREATE INDEX "MatchRoom_created_by_idx" ON "MatchRoom"("created_by");

-- ============ MatchSchedule ============
CREATE TABLE "MatchSchedule" (
    "id" SERIAL PRIMARY KEY,
    "room_id" INTEGER NOT NULL REFERENCES "MatchRoom"("id") ON DELETE CASCADE,
    "player1_osuId" TEXT NOT NULL,
    "player1_username" TEXT NOT NULL,
    "player2_osuId" TEXT NOT NULL,
    "player2_username" TEXT NOT NULL,
    "season" TEXT,
    "red_score" INTEGER NOT NULL DEFAULT 0,
    "blue_score" INTEGER NOT NULL DEFAULT 0,
    "status" "ScheduleStatus" NOT NULL DEFAULT 'pending',
    "replay_link" TEXT,
    "match_link" TEXT,
    "stream_link" TEXT,
    "created_by" TEXT NOT NULL,
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(3) NOT NULL
);
CREATE INDEX "MatchSchedule_room_id_idx" ON "MatchSchedule"("room_id");
CREATE INDEX "MatchSchedule_player1_osuId_idx" ON "MatchSchedule"("player1_osuId");
CREATE INDEX "MatchSchedule_player2_osuId_idx" ON "MatchSchedule"("player2_osuId");
CREATE INDEX "MatchSchedule_status_idx" ON "MatchSchedule"("status");
CREATE INDEX "MatchSchedule_season_idx" ON "MatchSchedule"("season");
CREATE INDEX "MatchSchedule_created_by_idx" ON "MatchSchedule"("created_by");

-- ============ PlayerMatchup ============
CREATE TABLE "PlayerMatchup" (
    "id" SERIAL PRIMARY KEY,
    "room_id" INTEGER REFERENCES "MatchRoom"("id") ON DELETE CASCADE,
    "player1_osuId" TEXT NOT NULL,
    "player1_username" TEXT NOT NULL,
    "player2_osuId" TEXT NOT NULL,
    "player2_username" TEXT NOT NULL,
    "status" "MatchupStatus" NOT NULL DEFAULT 'available',
    "created_by" TEXT NOT NULL,
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(3) NOT NULL
);
CREATE INDEX "PlayerMatchup_room_id_idx" ON "PlayerMatchup"("room_id");
CREATE INDEX "PlayerMatchup_player1_osuId_idx" ON "PlayerMatchup"("player1_osuId");
CREATE INDEX "PlayerMatchup_player2_osuId_idx" ON "PlayerMatchup"("player2_osuId");
CREATE INDEX "PlayerMatchup_status_idx" ON "PlayerMatchup"("status");
CREATE INDEX "PlayerMatchup_created_by_idx" ON "PlayerMatchup"("created_by");

-- ============ Message ============
CREATE TABLE "Message" (
    "id" SERIAL PRIMARY KEY,
    "sender_osuId" TEXT NOT NULL,
    "sender_username" TEXT NOT NULL,
    "receiver_osuId" TEXT NOT NULL,
    "receiver_username" TEXT NOT NULL,
    "type" "MessageType" NOT NULL,
    "title" TEXT NOT NULL,
    "content" TEXT NOT NULL,
    "related_matchup_id" INTEGER,
    "status" "MessageStatus" NOT NULL DEFAULT 'unread',
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(3) NOT NULL
);
CREATE INDEX "Message_sender_osuId_idx" ON "Message"("sender_osuId");
CREATE INDEX "Message_receiver_osuId_idx" ON "Message"("receiver_osuId");
CREATE INDEX "Message_type_idx" ON "Message"("type");
CREATE INDEX "Message_status_idx" ON "Message"("status");
CREATE INDEX "Message_related_matchup_id_idx" ON "Message"("related_matchup_id");

-- ============ StaffRoomAssignment ============
CREATE TABLE "StaffRoomAssignment" (
    "id" SERIAL PRIMARY KEY,
    "room_id" INTEGER NOT NULL REFERENCES "MatchRoom"("id") ON DELETE CASCADE,
    "staff_osuId" TEXT NOT NULL,
    "staff_username" TEXT NOT NULL,
    "role" "StaffRole" NOT NULL,
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(3) NOT NULL
);
CREATE INDEX "StaffRoomAssignment_room_id_idx" ON "StaffRoomAssignment"("room_id");
CREATE INDEX "StaffRoomAssignment_staff_osuId_idx" ON "StaffRoomAssignment"("staff_osuId");
CREATE INDEX "StaffRoomAssignment_role_idx" ON "StaffRoomAssignment"("role");

-- ============ MatchScore ============
CREATE TABLE "MatchScore" (
    "id" SERIAL PRIMARY KEY,
    "schedule_id" INTEGER NOT NULL REFERENCES "MatchSchedule"("id") ON DELETE CASCADE,
    "player_osuId" TEXT NOT NULL,
    "player_username" TEXT NOT NULL,
    "score" INTEGER NOT NULL,
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(3) NOT NULL
);
CREATE INDEX "MatchScore_schedule_id_idx" ON "MatchScore"("schedule_id");
CREATE INDEX "MatchScore_player_osuId_idx" ON "MatchScore"("player_osuId");

-- ============ TournamentSetting ============
CREATE TABLE "TournamentSetting" (
    "id" SERIAL PRIMARY KEY,
    "setting_key" TEXT NOT NULL UNIQUE,
    "setting_value" TEXT NOT NULL,
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP(3) NOT NULL
);

-- ============ MapSelection ============
CREATE TABLE "MapSelection" (
    "id" SERIAL PRIMARY KEY,
    "beatmapId" INTEGER NOT NULL,
    "beatmapsetId" INTEGER NOT NULL,
    "title" TEXT NOT NULL,
    "title_unicode" TEXT,
    "artist" TEXT NOT NULL,
    "artist_unicode" TEXT,
    "version" TEXT NOT NULL,
    "creator" TEXT NOT NULL,
    "starRating" DOUBLE PRECISION NOT NULL,
    "bpm" DOUBLE PRECISION NOT NULL,
    "totalLength" INTEGER NOT NULL,
    "maxCombo" INTEGER NOT NULL,
    "ar" DOUBLE PRECISION NOT NULL,
    "cs" DOUBLE PRECISION NOT NULL,
    "od" DOUBLE PRECISION NOT NULL,
    "hp" DOUBLE PRECISION NOT NULL,
    "selectedMods" TEXT,
    "modPosition" INTEGER,
    "comment" TEXT,
    "selectedBy" TEXT NOT NULL,
    "selectedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "season" TEXT,
    "category" TEXT,
    "url" TEXT,
    "coverUrl" TEXT,
    "approved" BOOLEAN NOT NULL DEFAULT false,
    "padding" BOOLEAN NOT NULL DEFAULT false,
    "isCustome" BOOLEAN NOT NULL DEFAULT false,
    "isOrigin" BOOLEAN NOT NULL DEFAULT false,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL
);
CREATE INDEX "MapSelection_beatmapId_idx" ON "MapSelection"("beatmapId");
CREATE INDEX "MapSelection_beatmapsetId_idx" ON "MapSelection"("beatmapsetId");
CREATE INDEX "MapSelection_selectedBy_idx" ON "MapSelection"("selectedBy");
CREATE INDEX "MapSelection_season_idx" ON "MapSelection"("season");
CREATE INDEX "MapSelection_category_idx" ON "MapSelection"("category");
CREATE INDEX "MapSelection_approved_idx" ON "MapSelection"("approved");

-- ============ MapComment ============
CREATE TABLE "MapComment" (
    "id" SERIAL PRIMARY KEY,
    "mapSelectionId" INTEGER NOT NULL REFERENCES "MapSelection"("id") ON DELETE CASCADE,
    "userId" TEXT NOT NULL,
    "username" TEXT NOT NULL,
    "avatar_url" TEXT NOT NULL DEFAULT '',
    "comment" TEXT NOT NULL,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL
);
CREATE INDEX "MapComment_mapSelectionId_idx" ON "MapComment"("mapSelectionId");
CREATE INDEX "MapComment_userId_idx" ON "MapComment"("userId");
CREATE INDEX "MapComment_createdAt_idx" ON "MapComment"("createdAt");

-- ============ 默认比赛设置 ============
INSERT INTO "TournamentSetting" ("setting_key", "setting_value", "updated_at") VALUES
    ('tournament_name', '', CURRENT_TIMESTAMP),
    ('max_pp_for_registration', '7000', CURRENT_TIMESTAMP),
    ('min_pp_for_registration', '0', CURRENT_TIMESTAMP),
    ('current_season', 's1', CURRENT_TIMESTAMP),
    ('current_season_stage', 'registration', CURRENT_TIMESTAMP),
    ('registration_enabled', 'true', CURRENT_TIMESTAMP),
    ('mappool_visible', 'false', CURRENT_TIMESTAMP);

-- ============ Bot 身份绑定 ============

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

-- 私聊一键绑定链接
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

-- ============ News（资讯，原前端 src/data/news/*.md 迁入）============
CREATE TABLE IF NOT EXISTS news (
    id SERIAL PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    date TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL,
    published BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS news_published_date_idx ON news (published, date);

-- ============ SeasonRule（赛季规则，原 ascBackend/rules/*.md 迁入）============
CREATE TABLE IF NOT EXISTS season_rules (
    season TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
