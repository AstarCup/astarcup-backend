// AstrBot 专用接口（服务端到服务端调用，使用 X-Bot-Token 鉴权）
//
// 身份模型：bot token 只证明“请求来自机器人”，具体选手身份必须来自
//   bot_platform_bindings（platform + platformUserId -> osu_id）。
// 绑定码由网站登录用户生成，机器人私聊兑换；不使用 users.qq 做鉴权。
//
//   GET  /api/bot/rules?season=                     比赛规则与赛事信息
//   GET  /api/bot/settings                          全局比赛设置
//   GET  /api/bot/mappool?season=&category=         公开图池（visible + approved）
//   GET  /api/bot/rooms?all=&round=                 房间列表（默认只返回可预约房间）
//   GET  /api/bot/whoami?platform=&platformUserId=  当前平台身份绑定了谁
//   GET  /api/bot/players?platform=&platformUserId= 选手档案 / 可预约对战 / 下一场
//   GET  /api/bot/messages?platform=&platformUserId= 收到的消息
//   GET  /api/bot/schedules?platform=&platformUserId= 赛程列表（管理员）
//   POST /api/bot/scores                             记录比分（管理员）
//   POST /api/bot/bind                              用一次性绑定码绑定平台身份
//   POST /api/bot/bind-link                         生成私聊一键绑定链接
//   POST /api/bot/book                              预约房间（必须带 matchupId；只带 roomId 时返回对战安排）
//   POST /api/bot/book/respond                      接受/拒绝预约邀请
//
// 网站侧（SessionUser）：
//   POST   /api/me/bot-bind-code             生成一次性绑定码
//   POST   /api/me/bot-bind-link             查看绑定链接（不消耗）
//   POST   /api/me/bot-bind-link/confirm     确认绑定（消耗链接）
//   GET    /api/me/bot-bindings              查看已绑定平台身份
//   DELETE /api/me/bot-bindings/{id}         解绑
use std::collections::{BTreeMap, HashMap};

use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::StatusCode;
use axum::http::header;
use axum::http::request::Parts;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use rand::RngCore;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::db::run_db;
use crate::enums::{
    MatchupStatus, MessageStatus, MessageType, RegistrationStatus, RoomStatus, ScheduleStatus,
    UserGroup,
};
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::{
    BotBindCode, BotBindLink, BotPlatformBinding, MapSelection, MatchRoom, MatchSchedule, Message,
    PlayerMatchup, User,
};
use crate::routes::messages_routes::{apply_message_action, load_message};
use crate::routes::schedules_routes::{claimed_room_ids, load_bookable_room, notify_booking};
use crate::schema::{
    bot_bind_codes, bot_bind_links, bot_platform_bindings, map_selections, match_rooms,
    match_schedules, messages, player_matchups, season_rules, tournament_settings, users,
};
use crate::state::AppState;

const BIND_CODE_TTL_MINUTES: i64 = 10;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/bot/rules", get(bot_rules))
        .route("/api/bot/settings", get(bot_settings))
        .route("/api/bot/mappool", get(bot_mappool))
        .route("/api/bot/rooms", get(bot_rooms))
        .route("/api/bot/whoami", get(bot_whoami))
        .route("/api/bot/players", get(bot_player))
        .route("/api/bot/messages", get(bot_messages))
        .route("/api/bot/schedules", get(bot_schedules))
        .route("/api/bot/scores", post(bot_record_score))
        .route("/api/bot/bind", post(bot_bind))
        .route("/api/bot/bind-link", post(create_bot_bind_link))
        .route("/api/bot/book", post(bot_book))
        .route("/api/bot/book/respond", post(bot_respond))
        .route("/api/me/bot-bind-code", post(create_bot_bind_code))
        .route("/api/me/bot-bind-link", post(inspect_bot_bind_link))
        .route("/api/me/bot-bind-link/confirm", post(confirm_bot_bind_link))
        .route("/api/me/bot-bindings", get(list_bot_bindings))
        .route("/api/me/bot-bindings/{id}", delete(delete_bot_binding))
}

// ============ 鉴权 ============

/// Bot 专用鉴权：X-Bot-Token 或 Authorization: Bearer <token>
pub struct BotAuth;

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

impl FromRequestParts<AppState> for BotAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let expected = state.config.bot_token.trim();
        if expected.is_empty() {
            return Err(AppError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "Bot 接口未启用（后端未配置 BOT_TOKEN）",
            ));
        }
        let provided = parts
            .headers
            .get("x-bot-token")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .or_else(|| {
                parts
                    .headers
                    .get(header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.strip_prefix("Bearer "))
                    .map(str::trim)
            });
        match provided {
            Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => Ok(BotAuth),
            _ => Err(AppError::unauthorized("无效的 Bot Token")),
        }
    }
}

// ============ 工具函数 ============

fn round_label(round: i32) -> &'static str {
    match round {
        1 => "QUA",
        2 => "RO16",
        3 => "QF",
        4 => "SF",
        5 => "F",
        6 => "GF",
        _ => "Stage",
    }
}

/// 设置值可能是裸字符串，也可能是 JSON 编码的字符串
fn setting_str(raw: &str) -> String {
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::String(s)) => s,
        _ => raw.to_string(),
    }
}

fn setting_bool(raw: Option<&String>) -> bool {
    raw.map(|r| match serde_json::from_str::<Value>(r) {
        Ok(Value::Bool(b)) => b,
        _ => r.trim().eq_ignore_ascii_case("true"),
    })
    .unwrap_or(false)
}

fn setting_number(raw: Option<&String>) -> Option<f64> {
    raw.and_then(|r| match serde_json::from_str::<Value>(r) {
        Ok(Value::Number(n)) => n.as_f64(),
        _ => r.trim().parse::<f64>().ok(),
    })
}

async fn load_settings_map(state: &AppState) -> Result<HashMap<String, String>, AppError> {
    let rows = run_db(&state.pool, |conn| {
        tournament_settings::table
            .load::<crate::models::TournamentSetting>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.setting_key, r.setting_value))
        .collect())
}

/// 设置里的阶段短码 / 图池类别长值 -> 图池类别
fn stage_to_category(stage: &str) -> Option<&'static str> {
    match stage.to_ascii_lowercase().as_str() {
        "qua" | "qualification" => Some("qualification"),
        "ro32" => Some("ro32"),
        "ro16" => Some("ro16"),
        "qf" | "quarterfinals" => Some("quarterfinals"),
        "sf" | "semifinals" => Some("semifinals"),
        "f" | "finals" => Some("finals"),
        "gf" | "grandfinals" => Some("grandfinals"),
        _ => None,
    }
}

fn stage_label(stage: &str) -> String {
    match stage.to_ascii_lowercase().as_str() {
        "qua" | "qualification" => "QUA".to_string(),
        "ro32" => "RO32".to_string(),
        "ro16" => "RO16".to_string(),
        "qf" | "quarterfinals" => "QF".to_string(),
        "sf" | "semifinals" => "SF".to_string(),
        "f" | "finals" => "F".to_string(),
        "gf" | "grandfinals" => "GF".to_string(),
        "registration" => "Registration".to_string(),
        other => other.to_uppercase(),
    }
}

fn category_label(category: &str) -> &'static str {
    match category {
        "qualification" => "QUA",
        "ro32" => "RO32",
        "ro16" => "RO16",
        "quarterfinals" => "QF",
        "semifinals" => "SF",
        "finals" => "F",
        "grandfinals" => "GF",
        _ => "Stage",
    }
}

fn category_rank(category: &str) -> usize {
    match category {
        "qualification" => 0,
        "ro32" => 1,
        "ro16" => 2,
        "quarterfinals" => 3,
        "semifinals" => 4,
        "finals" => 5,
        "grandfinals" => 6,
        _ => 99,
    }
}

fn mod_rank(mod_name: &str) -> usize {
    const ORDER: &[&str] = &["NM", "HD", "HR", "DT", "FM", "LZ", "TB"];
    ORDER.iter().position(|m| *m == mod_name).unwrap_or(99)
}

/// 图池类别输入：all/全部 表示不过滤；支持短码和长值。
fn normalize_category(input: &str) -> Result<Option<String>, AppError> {
    let value = input.trim().to_ascii_lowercase();
    if value.is_empty() || value == "all" || value == "全部" {
        return Ok(None);
    }
    let category = match value.as_str() {
        "qua" | "qualification" | "资格赛" => "qualification",
        "ro32" | "三十二强" => "ro32",
        "ro16" | "十六强" => "ro16",
        "qf" | "quarterfinals" | "八强" | "四分之一决赛" => "quarterfinals",
        "sf" | "semifinals" | "半决赛" => "semifinals",
        "f" | "finals" | "决赛" => "finals",
        "gf" | "grandfinals" | "总决赛" => "grandfinals",
        _ => {
            return Err(AppError::bad_request(
                "无效的图池阶段，可选：all/QUA/RO16/QF/SF/F/GF",
            ));
        }
    };
    Ok(Some(category.to_string()))
}

fn seasons_list(raw: Option<&String>) -> Value {
    let mut list: Vec<Value> = raw
        .and_then(|r| serde_json::from_str::<Value>(r).ok())
        .and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .map(|(id, v)| {
                        json!({
                            "id": id,
                            "name": v.get("name").and_then(|n| n.as_str()).unwrap_or(id),
                            "status": v.get("status").and_then(|s| s.as_str()).unwrap_or(""),
                        })
                    })
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    list.sort_by(|a, b| {
        a["id"]
            .as_str()
            .unwrap_or("")
            .cmp(b["id"].as_str().unwrap_or(""))
    });
    Value::Array(list)
}

/// 去掉 Markdown 的 YAML frontmatter，避免把元信息喂给模型
fn strip_front_matter(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines().peekable();
    let has_front_matter = lines
        .peek()
        .map(|line| line.trim_end() == "---")
        .unwrap_or(false);
    if !has_front_matter {
        return text.to_string();
    }
    lines.next();
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
    }
    if closed {
        lines
            .collect::<Vec<_>>()
            .join("\n")
            .trim_start()
            .to_string()
    } else {
        text.to_string()
    }
}

/// 防止 season 参数被用于路径穿越
pub(crate) fn valid_season_id(season: &str) -> bool {
    !season.is_empty()
        && !season.eq_ignore_ascii_case("all")
        && season != "全部"
        && season.len() <= 32
        && season
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn room_summary(room: &MatchRoom, claimed: bool, time_ok: bool) -> Value {
    json!({
        "id": room.id,
        "roomName": room.room_name,
        "roundNumber": room.round_number,
        "roundLabel": round_label(room.round_number),
        "matchNumber": room.match_number,
        "matchDate": room.match_date.format("%Y-%m-%d").to_string(),
        "matchTime": room.match_time.format("%Y-%m-%d %H:%M").to_string(),
        "status": room.status.as_str(),
        "claimed": claimed,
        "available": room.status == RoomStatus::Open && !claimed && time_ok,
        "description": room.description,
    })
}

/// 房间是否还没到「距开赛 2 小时」的时间点
fn time_allows_booking(room: &MatchRoom) -> bool {
    room.match_time > Utc::now().naive_utc() + chrono::Duration::hours(2)
}

fn matchup_summary(matchup: &PlayerMatchup, osu_id: &str, rooms: &HashMap<i32, Value>) -> Value {
    let is_player1 = matchup.player1_osu_id == osu_id;
    let is_player2 = matchup.player2_osu_id == osu_id;
    let is_participant = is_player1 || is_player2;
    let (opponent_id, opponent_name) = if is_player1 {
        (
            matchup.player2_osu_id.as_str(),
            matchup.player2_username.as_str(),
        )
    } else if is_player2 {
        (
            matchup.player1_osu_id.as_str(),
            matchup.player1_username.as_str(),
        )
    } else {
        ("", "")
    };
    let my_side = if is_player1 {
        json!("red")
    } else if is_player2 {
        json!("blue")
    } else {
        Value::Null
    };
    let opponent = if is_participant {
        json!({ "osuId": opponent_id, "username": opponent_name })
    } else {
        Value::Null
    };

    json!({
        "id": matchup.id,
        "status": matchup.status.as_str(),
        "player1": { "osuId": matchup.player1_osu_id, "username": matchup.player1_username },
        "player2": { "osuId": matchup.player2_osu_id, "username": matchup.player2_username },
        "opponent": opponent,
        "mySide": my_side,
        "isParticipant": is_participant,
        "room": matchup.room_id.and_then(|id| rooms.get(&id)),
        "createdAt": matchup.created_at.and_utc().to_rfc3339(),
    })
}

/// 平台身份参数（platform + platformUserId）
#[derive(Deserialize)]
struct PlatformQuery {
    platform: String,
    #[serde(rename = "platformUserId", alias = "platform_user_id")]
    platform_user_id: String,
}

/// 绑定码有效期 10 分钟，32 位随机十六进制
fn generate_bind_code() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_bind_code(code: &str) -> String {
    let digest = Sha256::digest(code.trim().as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn validate_platform_identity(platform: &str, platform_user_id: &str) -> Result<(), AppError> {
    let platform = platform.trim();
    let user_id = platform_user_id.trim();
    if platform.is_empty() || platform.len() > 64 {
        return Err(AppError::bad_request("无效的 platform"));
    }
    if !platform
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
    {
        return Err(AppError::bad_request("platform 只能包含字母、数字和 _-.:"));
    }
    if user_id.is_empty() || user_id.len() > 128 {
        return Err(AppError::bad_request("无效的 platformUserId"));
    }
    if user_id.chars().any(char::is_control) {
        return Err(AppError::bad_request("platformUserId 包含非法字符"));
    }
    Ok(())
}

/// 查询平台身份绑定及对应选手。未绑定时返回 None（不视为错误）。
async fn lookup_binding(
    state: &AppState,
    platform: &str,
    platform_user_id: &str,
) -> Result<Option<(BotPlatformBinding, User)>, AppError> {
    validate_platform_identity(platform, platform_user_id)?;
    let platform = platform.trim().to_string();
    let platform_user_id = platform_user_id.trim().to_string();
    run_db(&state.pool, move |conn| {
        let binding = bot_platform_bindings::table
            .filter(bot_platform_bindings::platform.eq(&platform))
            .filter(bot_platform_bindings::platform_user_id.eq(&platform_user_id))
            .first::<BotPlatformBinding>(conn)
            .optional()
            .map_err(AppError::from)?;
        let Some(binding) = binding else {
            return Ok::<_, AppError>(None);
        };
        let user = users::table
            .filter(users::osu_id.eq(&binding.osu_id))
            .first::<User>(conn)
            .optional()
            .map_err(AppError::from)?;
        Ok(user.map(|user| (binding, user)))
    })
    .await
}

/// 所有需要身份的操作都通过绑定关系解析选手，绝不接受客户端直接传 osuId。
async fn resolve_bound_user(
    state: &AppState,
    platform: &str,
    platform_user_id: &str,
) -> Result<User, AppError> {
    lookup_binding(state, platform, platform_user_id)
        .await?
        .map(|(_, user)| user)
        .ok_or_else(|| {
            AppError::forbidden(
                "该账号尚未绑定 astarcup。请私聊机器人发送 /bind 获取一次性绑定链接",
            )
        })
}

fn ensure_can_book(user: &User) -> Result<(), AppError> {
    if matches!(
        user.registration_status,
        RegistrationStatus::Registered | RegistrationStatus::Approved
    ) {
        Ok(())
    } else {
        Err(AppError::forbidden("该账号尚未完成报名或审核未通过"))
    }
}

fn require_admin(user: &User) -> Result<(), AppError> {
    if user.user_group == UserGroup::Admin {
        Ok(())
    } else {
        Err(AppError::forbidden("需要管理员权限"))
    }
}

async fn available_matchups(
    state: &AppState,
    osu_id: &str,
) -> Result<Vec<PlayerMatchup>, AppError> {
    let osu_id = osu_id.to_string();
    run_db(&state.pool, move |conn| {
        player_matchups::table
            .filter(
                player_matchups::player1_osu_id
                    .eq(&osu_id)
                    .or(player_matchups::player2_osu_id.eq(&osu_id)),
            )
            .filter(player_matchups::status.eq(MatchupStatus::Available))
            .order(player_matchups::created_at.asc())
            .load::<PlayerMatchup>(conn)
            .map_err(AppError::from)
    })
    .await
}

// ============ GET /api/bot/rules ============

#[derive(Deserialize)]
struct RulesQuery {
    season: Option<String>,
}

async fn bot_rules(
    State(state): State<AppState>,
    _bot: BotAuth,
    Query(q): Query<RulesQuery>,
) -> ApiResult<Json<Value>> {
    let settings = load_settings_map(&state).await?;

    let current_season = settings
        .get("current_season")
        .map(|s| setting_str(s).trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "s1".to_string());
    let season = q
        .season
        .as_deref()
        .map(setting_str)
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or(current_season);

    let season_name = settings
        .get("seasons")
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|v| {
            v.get(&season)
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str())
                .map(str::to_string)
        });

    // 规则文本优先级：tournament_settings.rules（全局覆盖） → season_rules 表 → BOT_RULES_DIR/{season}.md 文件兜底
    let mut rules_markdown = settings
        .get("rules")
        .map(|r| strip_front_matter(&setting_str(r)))
        .filter(|s| !s.trim().is_empty());
    let mut rules_source = if rules_markdown.is_some() {
        "settings"
    } else {
        "none"
    };
    if rules_markdown.is_none() && valid_season_id(&season) {
        let s = season.clone();
        let from_db = run_db(&state.pool, move |conn| {
            season_rules::table
                .filter(season_rules::season.eq(s))
                .select(season_rules::content)
                .first::<String>(conn)
                .optional()
                .map_err(AppError::from)
        })
        .await?;
        if let Some(text) = from_db {
            let cleaned = strip_front_matter(&text);
            if !cleaned.trim().is_empty() {
                rules_markdown = Some(cleaned);
                rules_source = "database";
            }
        }
    }
    if rules_markdown.is_none() && valid_season_id(&season) {
        let path = std::path::Path::new(&state.config.bot_rules_dir).join(format!("{season}.md"));
        if let Ok(text) = tokio::fs::read_to_string(&path).await {
            let cleaned = strip_front_matter(&text);
            if !cleaned.trim().is_empty() {
                rules_markdown = Some(cleaned);
                rules_source = "file";
            }
        }
    }

    let site = state.config.frontend_url.trim_end_matches('/').to_string();
    let tournaments = settings
        .get("seasons")
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or(Value::Null);

    // 与 /api/settings 的 default_settings 保持一致
    let registration_enabled = settings
        .get("registration_enabled")
        .map(|raw| setting_bool(Some(raw)))
        .unwrap_or(true);
    let stage = settings
        .get("current_season_stage")
        .map(|s| setting_str(s))
        .unwrap_or_else(|| "registration".to_string());
    let mappool_visible = settings
        .get("mappool_visible")
        .map(|raw| setting_bool(Some(raw)))
        .unwrap_or(false);

    Ok(ok(json!({
        "tournamentName": settings.get("tournament_name").map(|s| setting_str(s)).unwrap_or_default(),
        "season": season,
        "seasonName": season_name,
        "stage": stage,
        "seasons": tournaments,
        "registration": {
            "enabled": registration_enabled,
            "ppRange": {
                "min": setting_number(settings.get("min_pp_for_registration")).unwrap_or(0.0),
                "max": setting_number(settings.get("max_pp_for_registration")).unwrap_or(7000.0),
            },
        },
        "mappoolVisible": mappool_visible,
        "rulesMarkdown": rules_markdown,
        "rulesSource": rules_source,
        "bookingPolicy": {
            "timezone": "UTC+8",
            "minHoursBeforeStart": 2,
            "note": "距开赛不足 2 小时、房间已被安排、或对战已被预约时无法预约",
        },
        "site": {
            "guide": format!("{site}/guide"),
            "schedule": format!("{site}/schedule"),
            "mappool": format!("{site}/mappool"),
            "register": format!("{site}/register"),
        },
    })))
}

// ============ GET /api/bot/settings ============

async fn bot_settings(State(state): State<AppState>, _bot: BotAuth) -> ApiResult<Json<Value>> {
    let settings = load_settings_map(&state).await?;
    let current_season = settings
        .get("current_season")
        .map(|s| setting_str(s).trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "s1".to_string());
    let season_name = settings
        .get("seasons")
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|v| {
            v.get(&current_season)
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str())
                .map(str::to_string)
        });
    let stage = settings
        .get("current_season_stage")
        .map(|s| setting_str(s))
        .unwrap_or_else(|| "registration".to_string());
    let site = state.config.frontend_url.trim_end_matches('/').to_string();

    Ok(ok(json!({
        "tournamentName": settings.get("tournament_name").map(|s| setting_str(s)).unwrap_or_default(),
        "currentSeason": current_season,
        "currentSeasonName": season_name,
        "currentStage": stage,
        "currentStageLabel": stage_label(&stage),
        "seasons": seasons_list(settings.get("seasons")),
        "registration": {
            "enabled": settings
                .get("registration_enabled")
                .map(|r| setting_bool(Some(r)))
                .unwrap_or(true),
            "ppRange": {
                "min": setting_number(settings.get("min_pp_for_registration")).unwrap_or(0.0),
                "max": setting_number(settings.get("max_pp_for_registration")).unwrap_or(7000.0),
            },
        },
        "mappoolVisible": settings
            .get("mappool_visible")
            .map(|r| setting_bool(Some(r)))
            .unwrap_or(false),
        "bookingPolicy": {
            "timezone": "UTC+8",
            "minHoursBeforeStart": 2,
            "note": "距开赛不足 2 小时、房间已被安排、或对战已被预约时无法预约",
        },
        "site": {
            "home": site.clone(),
            "guide": format!("{site}/guide"),
            "schedule": format!("{site}/schedule"),
            "mappool": format!("{site}/mappool"),
            "register": format!("{site}/register"),
        },
    })))
}

// ============ GET /api/bot/mappool ============

#[derive(Deserialize)]
struct MappoolQuery {
    season: Option<String>,
    category: Option<String>,
}

async fn bot_mappool(
    State(state): State<AppState>,
    _bot: BotAuth,
    Query(q): Query<MappoolQuery>,
) -> ApiResult<Json<Value>> {
    let settings = load_settings_map(&state).await?;
    let current_season = settings
        .get("current_season")
        .map(|s| setting_str(s).trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "s1".to_string());
    let stage = settings
        .get("current_season_stage")
        .map(|s| setting_str(s))
        .unwrap_or_else(|| "registration".to_string());
    let visible = settings
        .get("mappool_visible")
        .map(|r| setting_bool(Some(r)))
        .unwrap_or(false);
    let season = q
        .season
        .as_deref()
        .map(setting_str)
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| current_season.clone());
    if !valid_season_id(&season) {
        return Err(AppError::bad_request("无效的赛季 ID"));
    }
    let season_name = settings
        .get("seasons")
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|v| {
            v.get(&season)
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str())
                .map(str::to_string)
        });
    let tournament_name = settings
        .get("tournament_name")
        .map(|s| setting_str(s))
        .unwrap_or_default();

    if !visible {
        return Ok(ok(json!({
            "visible": false,
            "tournamentName": tournament_name,
            "season": season,
            "seasonName": season_name,
            "message": "图池尚未公开",
        })));
    }

    let category = match q.category.as_deref() {
        Some(input) if !input.trim().is_empty() => normalize_category(input)?,
        _ => stage_to_category(&stage).map(str::to_string),
    };
    let category_label_value = category.as_deref().map(category_label);
    let season_for_db = season.clone();
    let category_for_db = category.clone();

    let rows = run_db(&state.pool, move |conn| {
        let mut query = map_selections::table
            .filter(map_selections::season.eq(&season_for_db))
            .filter(map_selections::approved.eq(true))
            .into_boxed();
        if let Some(c) = category_for_db {
            query = query.filter(map_selections::category.eq(c));
        }
        query
            .order((
                map_selections::category.asc(),
                map_selections::mod_position.asc(),
                map_selections::selected_at.asc(),
            ))
            .load::<MapSelection>(conn)
            .map_err(AppError::from)
    })
    .await?;

    let mut grouped: BTreeMap<String, BTreeMap<String, Vec<Value>>> = BTreeMap::new();
    let mut count = 0usize;
    for m in &rows {
        let category_value = m.category.clone().unwrap_or_else(|| "unknown".to_string());
        let mod_name = MapSelection::base_mod(m.selected_mods.as_deref().unwrap_or("NM"));
        let map = json!({
            "category": category_value,
            "categoryLabel": category_label(&category_value),
            "mod": mod_name,
            "position": m.mod_position.unwrap_or(0),
            "title": m.title,
            "artist": m.artist,
            "version": m.version,
            "creator": m.creator,
            "starRating": m.star_rating,
            "bpm": m.bpm,
            "lengthSeconds": m.total_length,
            "beatmapId": m.beatmap_id,
            "beatmapsetId": m.beatmapset_id,
            "url": m.url,
            "padding": m.padding,
            "comment": m.comment.clone().unwrap_or_default(),
        });
        count += 1;
        grouped
            .entry(category_value)
            .or_default()
            .entry(mod_name)
            .or_default()
            .push(map);
    }

    let groups: Vec<Value> = {
        let mut categories: Vec<(&String, &BTreeMap<String, Vec<Value>>)> =
            grouped.iter().collect();
        categories.sort_by_key(|(c, _)| category_rank(c));
        categories
            .into_iter()
            .map(|(category, mods)| {
                let mut mod_groups: Vec<(&String, &Vec<Value>)> = mods.iter().collect();
                mod_groups.sort_by_key(|(m, _)| mod_rank(m));
                json!({
                    "category": category,
                    "label": category_label(category),
                    "mods": mod_groups
                        .into_iter()
                        .map(|(mod_name, maps)| json!({
                            "mod": mod_name,
                            "maps": maps,
                        }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect()
    };

    let site = state.config.frontend_url.trim_end_matches('/');
    let mappool_url = format!("{site}/mappool?season={}", crate::auth::url_encode(&season));

    Ok(ok(json!({
        "visible": true,
        "tournamentName": tournament_name,
        "season": season,
        "seasonName": season_name,
        "stage": stage,
        "category": category,
        "categoryLabel": category_label_value,
        "count": count,
        "groups": groups,
        "site": { "mappool": mappool_url },
    })))
}

// ============ GET /api/bot/rooms ============

#[derive(Deserialize)]
struct RoomsQuery {
    #[serde(default)]
    all: bool,
    #[serde(default)]
    round: Option<i32>,
}

async fn bot_rooms(
    State(state): State<AppState>,
    _bot: BotAuth,
    Query(q): Query<RoomsQuery>,
) -> ApiResult<Json<Value>> {
    let include_all = q.all;
    let round = q.round;
    let rooms = run_db(&state.pool, move |conn| {
        let mut query = match_rooms::table.into_boxed();
        if !include_all {
            query = query.filter(match_rooms::status.eq(RoomStatus::Open));
        }
        if let Some(round) = round {
            query = query.filter(match_rooms::round_number.eq(round));
        }
        query
            .order(match_rooms::match_time.asc())
            .load::<MatchRoom>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let claimed = claimed_room_ids(&state).await?;

    let list: Vec<Value> = rooms
        .iter()
        .map(|r| room_summary(r, claimed.contains(&r.id), time_allows_booking(r)))
        .collect();
    let available_count = list
        .iter()
        .filter(|r| r["available"].as_bool().unwrap_or(false))
        .count();

    Ok(ok(json!({
        "timezone": "UTC+8",
        "total": list.len(),
        "availableCount": available_count,
        "rooms": list,
    })))
}

// ============ GET /api/bot/whoami ============

async fn bot_whoami(
    State(state): State<AppState>,
    _bot: BotAuth,
    Query(q): Query<PlatformQuery>,
) -> ApiResult<Json<Value>> {
    match lookup_binding(&state, &q.platform, &q.platform_user_id).await? {
        None => Ok(ok(json!({ "bound": false }))),
        Some((binding, user)) => Ok(ok(json!({
            "bound": true,
            "binding": binding.to_json(),
            "player": {
                "osuId": user.osu_id,
                "username": user.username,
                "registrationStatus": user.registration_status.as_str(),
                "canBook": matches!(
                    user.registration_status,
                    RegistrationStatus::Registered | RegistrationStatus::Approved
                ),
            },
        }))),
    }
}

// ============ GET /api/bot/players ============

async fn bot_player(
    State(state): State<AppState>,
    _bot: BotAuth,
    Query(q): Query<PlatformQuery>,
) -> ApiResult<Json<Value>> {
    let user = resolve_bound_user(&state, &q.platform, &q.platform_user_id).await?;
    let osu_id = user.osu_id.clone();
    let lookup = osu_id.clone();
    let (matchups, schedules, rooms) = run_db(&state.pool, move |conn| {
        let matchups = player_matchups::table
            .filter(
                player_matchups::player1_osu_id
                    .eq(&lookup)
                    .or(player_matchups::player2_osu_id.eq(&lookup)),
            )
            .order(player_matchups::created_at.asc())
            .load::<PlayerMatchup>(conn)
            .map_err(AppError::from)?;
        let schedules = match_schedules::table
            .filter(
                match_schedules::player1_osu_id
                    .eq(&lookup)
                    .or(match_schedules::player2_osu_id.eq(&lookup)),
            )
            .order(match_schedules::created_at.asc())
            .load::<MatchSchedule>(conn)
            .map_err(AppError::from)?;
        let rooms = match_rooms::table
            .load::<MatchRoom>(conn)
            .map_err(AppError::from)?;
        Ok::<_, AppError>((matchups, schedules, rooms))
    })
    .await?;

    let room_time_by_id: HashMap<i32, chrono::NaiveDateTime> =
        rooms.iter().map(|r| (r.id, r.match_time)).collect();
    let room_by_id: HashMap<i32, Value> = rooms.into_iter().map(|r| (r.id, r.to_json())).collect();
    let available: Vec<Value> = matchups
        .iter()
        .filter(|m| m.status == MatchupStatus::Available)
        .map(|m| matchup_summary(m, &osu_id, &room_by_id))
        .collect();
    let all_matchups: Vec<Value> = matchups
        .iter()
        .map(|m| matchup_summary(m, &osu_id, &room_by_id))
        .collect();

    let next_schedule = schedules
        .iter()
        .filter(|s| {
            matches!(
                s.status,
                ScheduleStatus::Pending | ScheduleStatus::Confirmed
            )
        })
        .filter_map(|s| room_time_by_id.get(&s.room_id).map(|time| (*time, s)))
        .min_by_key(|(time, _)| *time)
        .map(|(_, s)| s.to_json(room_by_id.get(&s.room_id)));

    let lookup = osu_id.clone();
    let pending_invitations = run_db(&state.pool, move |conn| {
        messages::table
            .filter(messages::receiver_osu_id.eq(&lookup))
            .filter(messages::message_type.eq(MessageType::MatchInvitation))
            .filter(messages::status.eq(MessageStatus::Unread))
            .order(messages::created_at.desc())
            .load::<Message>(conn)
            .map_err(AppError::from)
    })
    .await?;

    Ok(ok(json!({
        "osuId": user.osu_id,
        "username": user.username,
        "userGroup": user.user_group.as_str(),
        "registrationStatus": user.registration_status.as_str(),
        "season": user.season,
        "canBook": matches!(
            user.registration_status,
            RegistrationStatus::Registered | RegistrationStatus::Approved
        ),
        "availableMatchups": available,
        "matchups": all_matchups,
        "nextSchedule": next_schedule,
        "pendingInvitations": pending_invitations.iter().map(|m| m.to_json()).collect::<Vec<_>>(),
    })))
}

// ============ GET /api/bot/messages ============

async fn bot_messages(
    State(state): State<AppState>,
    _bot: BotAuth,
    Query(q): Query<PlatformQuery>,
) -> ApiResult<Json<Value>> {
    let user = resolve_bound_user(&state, &q.platform, &q.platform_user_id).await?;
    let osu_id = user.osu_id.clone();
    let list = run_db(&state.pool, move |conn| {
        messages::table
            .filter(messages::receiver_osu_id.eq(&osu_id))
            .order(messages::created_at.desc())
            .limit(20)
            .load::<Message>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok(json!({
        "count": list.len(),
        "messages": list.iter().map(|m| m.to_json()).collect::<Vec<_>>(),
    })))
}

// ============ GET /api/bot/schedules（管理员） ============

#[derive(Deserialize)]
struct BotSchedulesQuery {
    #[serde(flatten)]
    identity: PlatformQuery,
    status: Option<String>,
    season: Option<String>,
    limit: Option<i64>,
}

async fn bot_schedules(
    State(state): State<AppState>,
    _bot: BotAuth,
    Query(q): Query<BotSchedulesQuery>,
) -> ApiResult<Json<Value>> {
    let user =
        resolve_bound_user(&state, &q.identity.platform, &q.identity.platform_user_id).await?;
    require_admin(&user)?;

    let statuses = match q.status.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => {
            let mut parsed = Vec::new();
            for part in raw.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let status = ScheduleStatus::parse(part)
                    .ok_or_else(|| AppError::bad_request(format!("无效的状态：{part}")))?;
                parsed.push(status);
            }
            if parsed.is_empty() {
                None
            } else {
                Some(parsed)
            }
        }
        None => None,
    };
    let limit = q.limit.unwrap_or(30).clamp(1, 100);
    let season = q
        .season
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("all"))
        .map(|s| s.to_ascii_lowercase());
    if let Some(season) = &season {
        if !valid_season_id(season) {
            return Err(AppError::bad_request("无效的赛季 ID"));
        }
    }

    let (schedules, rooms) = run_db(&state.pool, move |conn| {
        let mut query = match_schedules::table.into_boxed();
        if let Some(statuses) = statuses {
            query = query.filter(match_schedules::status.eq_any(statuses));
        }
        if let Some(season) = season {
            query = query.filter(match_schedules::season.eq(season));
        }
        let schedules = query
            .order(match_schedules::created_at.desc())
            .limit(limit)
            .load::<MatchSchedule>(conn)
            .map_err(AppError::from)?;
        let rooms = match_rooms::table
            .load::<MatchRoom>(conn)
            .map_err(AppError::from)?;
        Ok::<_, AppError>((schedules, rooms))
    })
    .await?;

    let room_by_id: HashMap<i32, &MatchRoom> = rooms.iter().map(|r| (r.id, r)).collect();
    let list: Vec<Value> = schedules
        .iter()
        .map(|s| {
            let room = room_by_id.get(&s.room_id);
            json!({
                "id": s.id,
                "season": s.season,
                "roomId": s.room_id,
                "roomName": room.map(|r| r.room_name.clone()).unwrap_or_default(),
                "roundNumber": room.map(|r| r.round_number).unwrap_or(0),
                "roundLabel": room
                    .map(|r| round_label(r.round_number))
                    .unwrap_or("Stage"),
                "matchTime": room.map(|r| r.match_time.format("%Y-%m-%d %H:%M").to_string()),
                "player1": { "osuId": s.player1_osu_id, "username": s.player1_username },
                "player2": { "osuId": s.player2_osu_id, "username": s.player2_username },
                "redScore": s.red_score,
                "blueScore": s.blue_score,
                "status": s.status.as_str(),
            })
        })
        .collect();

    Ok(ok(json!({
        "count": list.len(),
        "timezone": "UTC+8",
        "schedules": list,
    })))
}

// ============ POST /api/bot/scores（管理员） ============

#[derive(Deserialize)]
struct RecordScoreBody {
    platform: String,
    #[serde(rename = "platformUserId", alias = "platform_user_id")]
    platform_user_id: String,
    #[serde(rename = "scheduleId", alias = "schedule_id")]
    schedule_id: i64,
    #[serde(rename = "redScore", alias = "red_score")]
    red_score: i64,
    #[serde(rename = "blueScore", alias = "blue_score")]
    blue_score: i64,
    #[serde(default)]
    season: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

async fn bot_record_score(
    State(state): State<AppState>,
    _bot: BotAuth,
    Json(body): Json<RecordScoreBody>,
) -> ApiResult<Json<Value>> {
    let user = resolve_bound_user(&state, &body.platform, &body.platform_user_id).await?;
    require_admin(&user)?;

    let schedule_id =
        i32::try_from(body.schedule_id).map_err(|_| AppError::bad_request("无效的赛程ID"))?;
    if !(0..=99).contains(&body.red_score) || !(0..=99).contains(&body.blue_score) {
        return Err(AppError::bad_request("比分必须在 0-99 之间"));
    }
    let red_score =
        i32::try_from(body.red_score).map_err(|_| AppError::bad_request("无效的红方比分"))?;
    let blue_score =
        i32::try_from(body.blue_score).map_err(|_| AppError::bad_request("无效的蓝方比分"))?;
    let status = match body
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => {
            ScheduleStatus::parse(raw).ok_or_else(|| AppError::bad_request("无效的赛程状态"))?
        }
        None => ScheduleStatus::Completed,
    };
    let season = body
        .season
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());
    if let Some(season) = &season {
        if !valid_season_id(season) {
            return Err(AppError::bad_request("无效的赛季 ID"));
        }
    }

    let now = Utc::now().naive_utc();
    let (schedule, room) = run_db(&state.pool, move |conn| {
        let affected = diesel::update(match_schedules::table.find(schedule_id))
            .set((
                season.map(|v| match_schedules::season.eq(v)),
                match_schedules::red_score.eq(red_score),
                match_schedules::blue_score.eq(blue_score),
                match_schedules::status.eq(status),
                match_schedules::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(AppError::from)?;
        if affected == 0 {
            return Err(AppError::not_found("赛程不存在"));
        }
        let schedule = match_schedules::table
            .find(schedule_id)
            .first::<MatchSchedule>(conn)
            .map_err(AppError::from)?;
        let room = match_rooms::table
            .find(schedule.room_id)
            .first::<MatchRoom>(conn)
            .optional()
            .map_err(AppError::from)?;
        Ok::<_, AppError>((schedule, room))
    })
    .await?;

    tracing::info!(
        "bot score recorded: admin_osu_id={} schedule_id={} score={}:{} status={}",
        user.osu_id,
        schedule.id,
        schedule.red_score,
        schedule.blue_score,
        schedule.status.as_str()
    );
    Ok(ok_msg(
        "比分已记录",
        json!({
            "scheduleId": schedule.id,
            "season": schedule.season,
            "redScore": schedule.red_score,
            "blueScore": schedule.blue_score,
            "status": schedule.status.as_str(),
            "player1": { "osuId": schedule.player1_osu_id, "username": schedule.player1_username },
            "player2": { "osuId": schedule.player2_osu_id, "username": schedule.player2_username },
            "room": room.map(|r| r.to_json()),
        }),
    ))
}

// ============ POST /api/bot/book ============

#[derive(Deserialize)]
struct BookBody {
    platform: String,
    #[serde(rename = "platformUserId", alias = "platform_user_id")]
    platform_user_id: String,
    #[serde(rename = "roomId", alias = "room_id")]
    room_id: i64,
    #[serde(rename = "matchupId", alias = "matchup_id", default)]
    matchup_id: Option<i64>,
}

async fn bot_book(
    State(state): State<AppState>,
    _bot: BotAuth,
    Json(body): Json<BookBody>,
) -> ApiResult<Json<Value>> {
    let user = resolve_bound_user(&state, &body.platform, &body.platform_user_id).await?;
    ensure_can_book(&user)?;
    let room_id = i32::try_from(body.room_id).map_err(|_| AppError::bad_request("无效的房间ID"))?;

    // 指定了对战：校验归属与状态；未指定：自动选择唯一一场可预约对战
    let matchup = if let Some(id) = body.matchup_id {
        let id = i32::try_from(id).map_err(|_| AppError::bad_request("无效的对战ID"))?;
        let osu_id = user.osu_id.clone();
        let matchup = run_db(&state.pool, move |conn| {
            player_matchups::table
                .find(id)
                .first::<PlayerMatchup>(conn)
                .optional()
                .map_err(AppError::from)
        })
        .await?
        .ok_or_else(|| AppError::not_found("对战不存在"))?;
        if matchup.player1_osu_id != osu_id && matchup.player2_osu_id != osu_id {
            return Err(AppError::forbidden("您不是该对战的参赛选手"));
        }
        if matchup.status != MatchupStatus::Available {
            return Err(AppError::bad_request("该对战已被预约或已完成"));
        }
        matchup
    } else {
        let list = available_matchups(&state, &user.osu_id).await?;
        if list.is_empty() {
            return Err(AppError::bad_request("当前没有可预约的对战"));
        }
        let rooms = run_db(&state.pool, |conn| {
            match_rooms::table
                .load::<MatchRoom>(conn)
                .map_err(AppError::from)
        })
        .await?;
        let room_by_id: HashMap<i32, Value> =
            rooms.into_iter().map(|r| (r.id, r.to_json())).collect();
        return Ok(ok_msg(
            "请先确认对战安排（红方=player1，蓝方=player2），然后带上 matchupId 再预约",
            json!({
                "needsMatchupChoice": true,
                "matchups": list
                    .iter()
                    .map(|m| matchup_summary(m, &user.osu_id, &room_by_id))
                    .collect::<Vec<_>>(),
            }),
        ));
    };

    let room = load_bookable_room(&state, room_id).await?;
    let (message_id, opponent_id, opponent_name) =
        notify_booking(&state, &matchup, &room, &user.osu_id, &user.username).await?;

    Ok(ok_msg(
        "预约成功，已发送确认请求给对手",
        json!({
            "matchupId": matchup.id,
            "messageId": message_id,
            "opponent": { "osuId": opponent_id, "username": opponent_name },
            "player1": { "osuId": matchup.player1_osu_id, "username": matchup.player1_username },
            "player2": { "osuId": matchup.player2_osu_id, "username": matchup.player2_username },
            "mySide": if matchup.player1_osu_id == user.osu_id { "red" } else { "blue" },
            "room": room_summary(&room, true, time_allows_booking(&room)),
            "timezone": "UTC+8",
        }),
    ))
}

// ============ POST /api/bot/book/respond ============

#[derive(Deserialize)]
struct RespondBody {
    platform: String,
    #[serde(rename = "platformUserId", alias = "platform_user_id")]
    platform_user_id: String,
    #[serde(rename = "messageId", alias = "message_id")]
    message_id: i64,
    action: String,
}

async fn bot_respond(
    State(state): State<AppState>,
    _bot: BotAuth,
    Json(body): Json<RespondBody>,
) -> ApiResult<Json<Value>> {
    if !matches!(body.action.as_str(), "accept" | "decline") {
        return Err(AppError::bad_request("action 只能是 accept 或 decline"));
    }
    let user = resolve_bound_user(&state, &body.platform, &body.platform_user_id).await?;
    let message_id =
        i32::try_from(body.message_id).map_err(|_| AppError::bad_request("无效的消息ID"))?;
    let message = load_message(&state, message_id).await?;
    apply_message_action(&state, &message, &user.osu_id, &body.action).await?;

    let mut data = json!({
        "messageId": message.id,
        "action": body.action,
        "matchupId": message.related_matchup_id,
    });
    if let Some(matchup_id) = message.related_matchup_id {
        let matchup = run_db(&state.pool, move |conn| {
            player_matchups::table
                .find(matchup_id)
                .first::<PlayerMatchup>(conn)
                .optional()
                .map_err(AppError::from)
        })
        .await?;
        if let Some(m) = matchup {
            data["matchupStatus"] = json!(m.status.as_str());
            data["roomId"] = json!(m.room_id);
        }
    }
    Ok(ok_msg(
        if body.action == "accept" {
            "已接受预约"
        } else {
            "已拒绝预约"
        },
        data,
    ))
}

// ============ 网站侧：生成绑定码 / 查看与解除绑定 ============

async fn create_bot_bind_code(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
) -> ApiResult<Json<Value>> {
    let code = generate_bind_code();
    let code_hash = hash_bind_code(&code);
    let osu_id = user.osu_id.clone();
    let now = Utc::now().naive_utc();
    let expires_at = now + chrono::Duration::minutes(BIND_CODE_TTL_MINUTES);

    run_db(&state.pool, move |conn| {
        conn.transaction::<_, AppError, _>(|conn| {
            // 同一账号只保留最新一个未使用的绑定码
            diesel::delete(
                bot_bind_codes::table
                    .filter(bot_bind_codes::osu_id.eq(&osu_id))
                    .filter(bot_bind_codes::used_at.is_null()),
            )
            .execute(conn)?;
            diesel::insert_into(bot_bind_codes::table)
                .values((
                    bot_bind_codes::code_hash.eq(&code_hash),
                    bot_bind_codes::osu_id.eq(&osu_id),
                    bot_bind_codes::expires_at.eq(expires_at),
                    bot_bind_codes::created_at.eq(now),
                ))
                .execute(conn)?;
            Ok(())
        })
    })
    .await?;

    tracing::info!("bot bind code issued for osu_id={}", user.osu_id);
    Ok(ok(json!({
        "code": code,
        "expiresAt": expires_at.and_utc().to_rfc3339(),
        "ttlMinutes": BIND_CODE_TTL_MINUTES,
        "command": format!("/bind {code}"),
    })))
}

async fn list_bot_bindings(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
) -> ApiResult<Json<Value>> {
    let osu_id = user.osu_id;
    let list = run_db(&state.pool, move |conn| {
        bot_platform_bindings::table
            .filter(bot_platform_bindings::osu_id.eq(&osu_id))
            .order(bot_platform_bindings::created_at.desc())
            .load::<BotPlatformBinding>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok(json!(
        list.iter().map(|b| b.to_json()).collect::<Vec<_>>()
    )))
}

async fn delete_bot_binding(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    let osu_id = user.osu_id.clone();
    let actor = user.osu_id;
    let affected = run_db(&state.pool, move |conn| {
        diesel::delete(
            bot_platform_bindings::table
                .filter(bot_platform_bindings::id.eq(id))
                .filter(bot_platform_bindings::osu_id.eq(&osu_id)),
        )
        .execute(conn)
        .map_err(AppError::from)
    })
    .await?;
    if affected == 0 {
        return Err(AppError::not_found("绑定不存在"));
    }
    tracing::info!("bot binding {} removed by osu_id={}", id, actor);
    Ok(ok_msg("已解绑", Value::Null))
}

// ============ POST /api/bot/bind ============

/// 写入/更新平台身份绑定；两边唯一约束的冲突会转成 409。
fn upsert_binding(
    conn: &mut PgConnection,
    platform: &str,
    platform_user_id: &str,
    display_name: Option<String>,
    osu_id: &str,
    now: chrono::NaiveDateTime,
) -> Result<(), AppError> {
    let existing = bot_platform_bindings::table
        .filter(bot_platform_bindings::platform.eq(platform))
        .filter(bot_platform_bindings::platform_user_id.eq(platform_user_id))
        .first::<BotPlatformBinding>(conn)
        .optional()?;
    if let Some(binding) = existing {
        if binding.osu_id != osu_id {
            return Err(AppError::conflict(
                "该平台账号已绑定其他选手，请先让原账号在网站解绑",
            ));
        }
        let new_display = display_name.or(binding.display_name);
        diesel::update(bot_platform_bindings::table.find(binding.id))
            .set((
                bot_platform_bindings::display_name.eq(new_display),
                bot_platform_bindings::updated_at.eq(now),
            ))
            .execute(conn)?;
        return Ok(());
    }

    let same_player = bot_platform_bindings::table
        .filter(bot_platform_bindings::platform.eq(platform))
        .filter(bot_platform_bindings::osu_id.eq(osu_id))
        .first::<BotPlatformBinding>(conn)
        .optional()?;
    if same_player.is_some() {
        return Err(AppError::conflict(
            "该选手已绑定其他平台账号，请先在网站解绑",
        ));
    }

    diesel::insert_into(bot_platform_bindings::table)
        .values((
            bot_platform_bindings::platform.eq(platform),
            bot_platform_bindings::platform_user_id.eq(platform_user_id),
            bot_platform_bindings::osu_id.eq(osu_id),
            bot_platform_bindings::display_name.eq(display_name),
            bot_platform_bindings::created_at.eq(now),
            bot_platform_bindings::updated_at.eq(now),
        ))
        .execute(conn)?;
    Ok(())
}

#[derive(Deserialize)]
struct BindBody {
    platform: String,
    #[serde(rename = "platformUserId", alias = "platform_user_id")]
    platform_user_id: String,
    #[serde(rename = "displayName", alias = "display_name", default)]
    display_name: Option<String>,
    code: String,
}

async fn bot_bind(
    State(state): State<AppState>,
    _bot: BotAuth,
    Json(body): Json<BindBody>,
) -> ApiResult<Json<Value>> {
    validate_platform_identity(&body.platform, &body.platform_user_id)?;
    let platform = body.platform.trim().to_string();
    let platform_user_id = body.platform_user_id.trim().to_string();
    let display_name = body
        .display_name
        .map(|s| s.trim().chars().take(128).collect::<String>())
        .filter(|s| !s.is_empty());
    let code_hash = hash_bind_code(&body.code);
    let now = Utc::now().naive_utc();

    let result = run_db(&state.pool, move |conn| {
        conn.transaction::<_, AppError, _>(|conn| {
            let code_row = bot_bind_codes::table
                .filter(bot_bind_codes::code_hash.eq(&code_hash))
                .first::<BotBindCode>(conn)
                .optional()?
                .ok_or_else(|| AppError::bad_request("绑定码无效"))?;
            if code_row.used_at.is_some() {
                return Err(AppError::bad_request("绑定码已使用，请重新生成"));
            }
            if code_row.expires_at <= now {
                return Err(AppError::bad_request("绑定码已过期，请重新生成"));
            }
            let user = users::table
                .filter(users::osu_id.eq(&code_row.osu_id))
                .first::<User>(conn)
                .optional()?
                .ok_or_else(|| AppError::not_found("绑定的选手不存在"))?;

            upsert_binding(
                conn,
                &platform,
                &platform_user_id,
                display_name,
                &code_row.osu_id,
                now,
            )?;

            // 带 used_at IS NULL 条件，避免同一绑定码并发兑换
            let marked = diesel::update(bot_bind_codes::table.find(code_row.id))
                .filter(bot_bind_codes::used_at.is_null())
                .set(bot_bind_codes::used_at.eq(now))
                .execute(conn)?;
            if marked == 0 {
                return Err(AppError::bad_request("绑定码已被使用，请重新生成"));
            }

            Ok(json!({
                "osuId": user.osu_id,
                "username": user.username,
                "platform": platform,
                "platformUserId": platform_user_id,
            }))
        })
    })
    .await?;

    tracing::info!(
        "bot bind: platform={} platform_user_id={} osu_id={}",
        result["platform"].as_str().unwrap_or("-"),
        result["platformUserId"].as_str().unwrap_or("-"),
        result["osuId"].as_str().unwrap_or("-"),
    );
    Ok(ok_msg("绑定成功", result))
}

// ============ 私聊一键绑定链接 ============

#[derive(Deserialize)]
struct BindLinkBody {
    platform: String,
    #[serde(rename = "platformUserId", alias = "platform_user_id")]
    platform_user_id: String,
    #[serde(rename = "displayName", alias = "display_name", default)]
    display_name: Option<String>,
}

/// POST /api/bot/bind-link —— 机器人为当前平台身份生成一次性绑定链接
async fn create_bot_bind_link(
    State(state): State<AppState>,
    _bot: BotAuth,
    Json(body): Json<BindLinkBody>,
) -> ApiResult<Json<Value>> {
    validate_platform_identity(&body.platform, &body.platform_user_id)?;
    let platform = body.platform.trim().to_string();
    let platform_user_id = body.platform_user_id.trim().to_string();
    let display_name = body
        .display_name
        .map(|s| s.trim().chars().take(128).collect::<String>())
        .filter(|s| !s.is_empty());

    let token = generate_bind_code();
    let token_hash = hash_bind_code(&token);
    let now = Utc::now().naive_utc();
    let expires_at = now + chrono::Duration::minutes(BIND_CODE_TTL_MINUTES);

    let platform_for_db = platform.clone();
    let user_id_for_db = platform_user_id.clone();
    run_db(&state.pool, move |conn| {
        conn.transaction::<_, AppError, _>(|conn| {
            // 同一平台身份只保留最新一个未使用链接
            diesel::delete(
                bot_bind_links::table
                    .filter(bot_bind_links::platform.eq(&platform_for_db))
                    .filter(bot_bind_links::platform_user_id.eq(&user_id_for_db))
                    .filter(bot_bind_links::used_at.is_null()),
            )
            .execute(conn)?;
            diesel::insert_into(bot_bind_links::table)
                .values((
                    bot_bind_links::token_hash.eq(&token_hash),
                    bot_bind_links::platform.eq(&platform_for_db),
                    bot_bind_links::platform_user_id.eq(&user_id_for_db),
                    bot_bind_links::display_name.eq(display_name),
                    bot_bind_links::expires_at.eq(expires_at),
                    bot_bind_links::created_at.eq(now),
                ))
                .execute(conn)?;
            Ok(())
        })
    })
    .await?;

    // token 放在 fragment 中：浏览器不会把它发给服务器，也不会进入访问日志
    let frontend = state.config.frontend_url.trim_end_matches('/');
    tracing::info!(
        "bot bind link issued: platform={} platform_user_id={}",
        platform,
        platform_user_id
    );
    Ok(ok(json!({
        "url": format!("{frontend}/bind#token={token}"),
        "expiresAt": expires_at.and_utc().to_rfc3339(),
        "ttlMinutes": BIND_CODE_TTL_MINUTES,
    })))
}

#[derive(Deserialize)]
struct BindLinkTokenBody {
    token: String,
}

/// POST /api/me/bot-bind-link —— 查看链接对应平台身份（不消耗 token）
async fn inspect_bot_bind_link(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Json(body): Json<BindLinkTokenBody>,
) -> ApiResult<Json<Value>> {
    let token_hash = hash_bind_code(&body.token);
    let now = Utc::now().naive_utc();
    let link = run_db(&state.pool, move |conn| {
        bot_bind_links::table
            .filter(bot_bind_links::token_hash.eq(&token_hash))
            .first::<BotBindLink>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?
    .ok_or_else(|| AppError::not_found("绑定链接无效"))?;

    if link.used_at.is_some() {
        return Err(AppError::bad_request(
            "绑定链接已使用，请在私聊重新发送 /bind",
        ));
    }
    if link.expires_at <= now {
        return Err(AppError::bad_request(
            "绑定链接已过期，请在私聊重新发送 /bind",
        ));
    }

    let platform = link.platform.clone();
    let platform_user_id = link.platform_user_id.clone();
    let osu_id = user.osu_id.clone();
    let already_bound = run_db(&state.pool, move |conn| {
        bot_platform_bindings::table
            .filter(bot_platform_bindings::platform.eq(&platform))
            .filter(bot_platform_bindings::platform_user_id.eq(&platform_user_id))
            .filter(bot_platform_bindings::osu_id.eq(&osu_id))
            .first::<BotPlatformBinding>(conn)
            .optional()
            .map_err(AppError::from)
            .map(|v| v.is_some())
    })
    .await?;

    Ok(ok(json!({
        "platform": link.platform,
        "platformUserId": link.platform_user_id,
        "displayName": link.display_name,
        "expiresAt": link.expires_at.and_utc().to_rfc3339(),
        "osuId": user.osu_id,
        "username": user.username,
        "alreadyBound": already_bound,
    })))
}

/// POST /api/me/bot-bind-link/confirm —— 登录用户确认绑定（消耗 token）
async fn confirm_bot_bind_link(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Json(body): Json<BindLinkTokenBody>,
) -> ApiResult<Json<Value>> {
    let token_hash = hash_bind_code(&body.token);
    let osu_id = user.osu_id.clone();
    let now = Utc::now().naive_utc();

    let result = run_db(&state.pool, move |conn| {
        conn.transaction::<_, AppError, _>(|conn| {
            let link = bot_bind_links::table
                .filter(bot_bind_links::token_hash.eq(&token_hash))
                .first::<BotBindLink>(conn)
                .optional()?
                .ok_or_else(|| AppError::not_found("绑定链接无效"))?;
            if link.used_at.is_some() {
                return Err(AppError::bad_request("绑定链接已使用，请重新获取"));
            }
            if link.expires_at <= now {
                return Err(AppError::bad_request(
                    "绑定链接已过期，请在私聊重新发送 /bind",
                ));
            }
            let db_user = users::table
                .filter(users::osu_id.eq(&osu_id))
                .first::<User>(conn)
                .optional()?
                .ok_or_else(|| AppError::unauthorized("用户不存在，请重新登录"))?;

            upsert_binding(
                conn,
                &link.platform,
                &link.platform_user_id,
                link.display_name.clone(),
                &osu_id,
                now,
            )?;

            let marked = diesel::update(bot_bind_links::table.find(link.id))
                .filter(bot_bind_links::used_at.is_null())
                .set(bot_bind_links::used_at.eq(now))
                .execute(conn)?;
            if marked == 0 {
                return Err(AppError::bad_request("绑定链接已被使用"));
            }

            Ok(json!({
                "osuId": db_user.osu_id,
                "username": db_user.username,
                "platform": link.platform,
                "platformUserId": link.platform_user_id,
            }))
        })
    })
    .await?;

    tracing::info!(
        "bot bind link confirmed: platform={} platform_user_id={} osu_id={}",
        result["platform"].as_str().unwrap_or("-"),
        result["platformUserId"].as_str().unwrap_or("-"),
        result["osuId"].as_str().unwrap_or("-"),
    );
    Ok(ok_msg("绑定成功", result))
}
