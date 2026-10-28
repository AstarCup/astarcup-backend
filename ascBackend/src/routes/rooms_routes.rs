// 比赛房间路由（合并原 match-rooms、match-rooms/delete、available-rooms-for-staff、
// match-scores/save+update 的查询与真实写入）
//
//   GET    /api/rooms?status=&withSchedules=true    房间列表（status=open 过滤供 staff 选房）
//   GET    /api/rooms/saved                         含赛程与分数统计的房间列表
//   POST   /api/rooms                               创建房间（管理员）
//   PATCH  /api/rooms/{id}                          修改房间赛季（管理员）
//   DELETE /api/rooms/{id}                          删除房间及其赛程（管理员）
//   GET    /api/rooms/{id}/scores                   房间的分数（scheduleIds → MatchScore）
//   POST   /api/rooms/{id}/scores                   保存分数（管理员，真实写库）
//
// 开赛前 2 小时的房间会自动置为 closed（后台定时任务 + 读取列表时兜底）。
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::db::run_db;
use crate::enums::RoomStatus;
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::{MatchRoom, MatchSchedule, MatchScore};
use crate::schema::{match_rooms, match_schedules, match_scores};
use crate::state::AppState;

/// 距开赛不足 2 小时的 open 房间自动关闭；返回本次关闭的房间数。
/// 幂等，列表读取与后台定时任务都会调用。
pub(crate) async fn close_due_rooms(state: &AppState) -> Result<usize, AppError> {
    run_db(&state.pool, |conn| {
        let now = Utc::now().naive_utc();
        diesel::update(match_rooms::table)
            .filter(match_rooms::status.eq(RoomStatus::Open))
            .filter(match_rooms::match_time.le(now + Duration::hours(2)))
            .set((
                match_rooms::status.eq(RoomStatus::Closed),
                match_rooms::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/rooms", get(list_rooms).post(create_room))
        .route("/api/rooms/saved", get(saved_rooms))
        .route(
            "/api/rooms/{id}",
            get(get_room).patch(update_room).delete(delete_room),
        )
        .route(
            "/api/rooms/{id}/scores",
            get(room_scores).post(save_room_scores),
        )
}

// ============ 查询参数 ============

#[derive(serde::Deserialize)]
struct RoomsQuery {
    status: Option<String>,
    /// 赛季过滤：不传 = 全部，值为赛季 ID 时只看该赛季
    season: Option<String>,
    #[serde(rename = "withSchedules", default)]
    with_schedules: bool,
}

// ============ GET /api/rooms ============

async fn list_rooms(
    State(state): State<AppState>,
    _user: crate::auth::SessionUser,
    Query(q): Query<RoomsQuery>,
) -> ApiResult<Json<Value>> {
    // 先把到点的房间关掉，保证列表里的状态实时
    close_due_rooms(&state).await?;
    let status_filter = match q.status.as_deref() {
        Some("open") => Some(RoomStatus::Open),
        Some("full") => Some(RoomStatus::Full),
        Some("closed") => Some(RoomStatus::Closed),
        _ => None,
    };
    let with_schedules = q.with_schedules;
    let season_filter = q
        .season
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty() && s != "all");

    let rooms = run_db(&state.pool, move |conn| {
        let mut query = match_rooms::table.into_boxed();
        if let Some(s) = status_filter {
            query = query.filter(match_rooms::status.eq(s));
        }
        if let Some(s) = season_filter {
            query = query.filter(match_rooms::season.eq(s));
        }
        query
            .order(match_rooms::created_at.desc())
            .load::<MatchRoom>(conn)
            .map_err(AppError::from)
    })
    .await?;

    let schedules_map: std::collections::HashMap<i32, Vec<Value>> = if with_schedules {
        let schedules = run_db(&state.pool, |conn| {
            match_schedules::table
                .load::<MatchSchedule>(conn)
                .map_err(AppError::from)
        })
        .await?;
        schedules
            .into_iter()
            .map(|s| (s.room_id, s.to_json(None)))
            .fold(std::collections::HashMap::new(), |mut acc, (room_id, v)| {
                acc.entry(room_id).or_default().push(v);
                acc
            })
    } else {
        Default::default()
    };

    let mut rooms_json: Vec<Value> = rooms
        .iter()
        .map(|r| {
            let mut v = r.to_json();
            if with_schedules {
                v["schedules"] = json!(schedules_map.get(&r.id).cloned().unwrap_or_default());
            }
            v
        })
        .collect();

    // 已被安排的房间：有赛程，或被 pending/scheduled 对战占用 —— 不可再被预约
    let claimed_ids = crate::routes::schedules_routes::claimed_room_ids(&state).await?;
    for v in &mut rooms_json {
        if let Some(id) = v.get("id").and_then(|x| x.as_i64()) {
            v["claimed"] = json!(claimed_ids.contains(&(id as i32)));
        }
    }

    Ok(ok(json!(rooms_json)))
}

// ============ GET /api/rooms/saved ============

/// 含赛程与分数统计的房间（原 match-scores/save GET 无 roomId 分支）
async fn saved_rooms(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    close_due_rooms(&state).await?;
    let (rooms, scores) = run_db(&state.pool, |conn| {
        let schedules = match_schedules::table
            .load::<MatchSchedule>(conn)
            .map_err(AppError::from)?;
        let all_scores = match_scores::table
            .load::<MatchScore>(conn)
            .map_err(AppError::from)?;
        let rooms = match_rooms::table
            .order(match_rooms::created_at.desc())
            .load::<MatchRoom>(conn)
            .map_err(AppError::from)?;
        Ok((rooms, (schedules, all_scores)))
    })
    .await?;
    let (schedules, all_scores) = scores;

    let mut rooms_json = Vec::new();
    for room in &rooms {
        let room_schedule_ids: Vec<i32> = schedules
            .iter()
            .filter(|s| s.room_id == room.id)
            .map(|s| s.id)
            .collect();
        let scores_count = all_scores
            .iter()
            .filter(|sc| room_schedule_ids.contains(&sc.schedule_id))
            .count() as i64;
        let mut room_json = room.to_json();
        room_json["scheduleIds"] = json!(room_schedule_ids);
        room_json["scoresCount"] = json!(scores_count);
        rooms_json.push(room_json);
    }
    Ok(ok(json!(rooms_json)))
}

// ============ GET /api/rooms/{id} ============

async fn get_room(
    State(state): State<AppState>,
    _user: crate::auth::SessionUser,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    let room = run_db(&state.pool, move |conn| {
        match_rooms::table
            .find(id)
            .first::<MatchRoom>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?;
    match room {
        Some(r) => Ok(ok(r.to_json())),
        None => Err(AppError::not_found("房间不存在")),
    }
}

// ============ POST /api/rooms ============

#[derive(serde::Deserialize)]
struct CreateRoomBody {
    room_name: String,
    round_number: i32,
    match_date: String,
    match_time: String,
    match_number: i32,
    max_participants: Option<i32>,
    description: Option<String>,
    /// 赛季 ID；不传时取系统设置里的 current_season
    season: Option<String>,
}

/// 解析前端传来的 ISO 日期时间字符串（容忍 Z 后缀）
fn parse_dt(s: &str) -> Result<NaiveDateTime, AppError> {
    let t = s.trim().trim_end_matches('Z');
    NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|_| {
            // 纯日期：时间归零
            NaiveDate::parse_from_str(t, "%Y-%m-%d").map(|d| d.and_time(NaiveTime::MIN))
        })
        .map_err(|_| AppError::bad_request(format!("无效的日期时间格式: {s}")))
}

async fn create_room(
    State(state): State<AppState>,
    admin: crate::auth::AdminUser,
    Json(body): Json<CreateRoomBody>,
) -> ApiResult<Json<Value>> {
    if body.room_name.is_empty() {
        return Err(AppError::bad_request("缺少必要字段"));
    }
    let match_date = parse_dt(&body.match_date)?;
    let match_time = parse_dt(&body.match_time)?;
    // 赛季：显式传入优先，否则落当前赛季，保证每个房间都有赛季归属
    let season = match body.season.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_ascii_lowercase(),
        _ => crate::routes::schedules_routes::current_season(&state).await?,
    };
    let room = run_db(&state.pool, move |conn| {
        let now = Utc::now().naive_utc();
        diesel::insert_into(match_rooms::table)
            .values((
                match_rooms::room_name.eq(&body.room_name),
                match_rooms::round_number.eq(body.round_number),
                match_rooms::match_date.eq(match_date),
                match_rooms::match_time.eq(match_time),
                match_rooms::match_number.eq(body.match_number),
                match_rooms::max_participants.eq(body.max_participants.unwrap_or(2)),
                match_rooms::status.eq(RoomStatus::Open),
                match_rooms::description.eq(body.description.unwrap_or_default()),
                match_rooms::created_by.eq(&admin.osu_id),
                match_rooms::created_at.eq(now),
                match_rooms::updated_at.eq(now),
                match_rooms::season.eq(season),
            ))
            .get_result::<MatchRoom>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok_msg("比赛房间创建成功", room.to_json()))
}

// ============ PATCH /api/rooms/{id} ============

#[derive(serde::Deserialize)]
struct UpdateRoomBody {
    /// 把房间改到另一个赛季；房间下的赛程会一起迁移，保持“赛程跟随房间”
    season: Option<String>,
}

async fn update_room(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(id): Path<i32>,
    Json(body): Json<UpdateRoomBody>,
) -> ApiResult<Json<Value>> {
    let season = body
        .season
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request("缺少赛季参数"))?;

    let room = run_db(&state.pool, move |conn| {
        conn.transaction::<MatchRoom, AppError, _>(|txn| {
            let now = Utc::now().naive_utc();
            let updated = diesel::update(match_rooms::table.find(id))
                .set((
                    match_rooms::season.eq(&season),
                    match_rooms::updated_at.eq(now),
                ))
                .execute(txn)
                .map_err(AppError::from)?;
            if updated == 0 {
                return Err(AppError::not_found("房间不存在"));
            }
            // 赛程跟随房间赛季
            diesel::update(match_schedules::table.filter(match_schedules::room_id.eq(id)))
                .set((
                    match_schedules::season.eq(&season),
                    match_schedules::updated_at.eq(now),
                ))
                .execute(txn)
                .map_err(AppError::from)?;
            match_rooms::table
                .find(id)
                .first::<MatchRoom>(txn)
                .map_err(AppError::from)
        })
    })
    .await?;
    Ok(ok_msg("房间赛季已更新", room.to_json()))
}

// ============ DELETE /api/rooms/{id} ============

async fn delete_room(
    State(state): State<AppState>,
    admin: crate::auth::AdminUser,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    let _ = admin;
    let room = run_db(&state.pool, move |conn| {
        let room = match_rooms::table
            .find(id)
            .first::<MatchRoom>(conn)
            .optional()
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("房间不存在"))?;
        // 关联赛程 → 分数 → 对战 → 分配，一并删除
        let schedule_ids: Vec<i32> = match_schedules::table
            .filter(match_schedules::room_id.eq(id))
            .select(match_schedules::id)
            .load(conn)
            .map_err(AppError::from)?;
        if !schedule_ids.is_empty() {
            diesel::delete(
                match_scores::table.filter(match_scores::schedule_id.eq_any(&schedule_ids)),
            )
            .execute(conn)
            .map_err(AppError::from)?;
            diesel::delete(match_schedules::table.filter(match_schedules::room_id.eq(id)))
                .execute(conn)
                .map_err(AppError::from)?;
        }
        diesel::delete(
            crate::schema::player_matchups::table
                .filter(crate::schema::player_matchups::room_id.eq(id)),
        )
        .execute(conn)
        .map_err(AppError::from)?;
        diesel::delete(
            crate::schema::staff_room_assignments::table
                .filter(crate::schema::staff_room_assignments::room_id.eq(id)),
        )
        .execute(conn)
        .map_err(AppError::from)?;
        diesel::delete(match_rooms::table.find(id))
            .execute(conn)
            .map_err(AppError::from)?;
        Ok(room)
    })
    .await?;
    Ok(ok_msg(
        &format!("房间 \"{}\" 及其所有预约已删除", room.room_name),
        Value::Null,
    ))
}

// ============ GET /api/rooms/{id}/scores ============

async fn room_scores(State(state): State<AppState>, Path(id): Path<i32>) -> ApiResult<Json<Value>> {
    let result = run_db(&state.pool, move |conn| {
        let room = match_rooms::table
            .find(id)
            .first::<MatchRoom>(conn)
            .optional()
            .map_err(AppError::from)?;
        let schedule_ids: Vec<i32> = match_schedules::table
            .filter(match_schedules::room_id.eq(id))
            .select(match_schedules::id)
            .load(conn)
            .map_err(AppError::from)?;
        let scores = if schedule_ids.is_empty() {
            vec![]
        } else {
            match_scores::table
                .filter(match_scores::schedule_id.eq_any(&schedule_ids))
                .load::<MatchScore>(conn)
                .map_err(AppError::from)?
        };
        Ok((room, scores))
    })
    .await?;
    let (room, scores) = result;
    Ok(ok(json!({
        "room": room.map(|r| r.to_json()).unwrap_or(Value::Null),
        "scores": scores.iter().map(|s| s.to_json()).collect::<Vec<_>>(),
    })))
}

// ============ POST /api/rooms/{id}/scores ============

#[derive(serde::Deserialize)]
struct ScoreItem {
    #[serde(rename = "scheduleId")]
    schedule_id: i32,
    #[serde(rename = "playerOsuId")]
    player_osu_id: String,
    #[serde(rename = "playerUsername")]
    player_username: String,
    score: i32,
}

#[derive(serde::Deserialize)]
struct SaveScoresBody {
    scores: Vec<ScoreItem>,
}

/// 保存分数（真实写库；替换原 match-scores/save 与 update 的假实现）
async fn save_room_scores(
    State(state): State<AppState>,
    admin: crate::auth::AdminUser,
    Path(room_id): Path<i32>,
    Json(body): Json<SaveScoresBody>,
) -> ApiResult<Json<Value>> {
    let _ = admin;
    let count = body.scores.len();
    let scores = body.scores;
    run_db(&state.pool, move |conn| {
        let now = Utc::now().naive_utc();
        conn.transaction(|txn| {
            for s in &scores {
                // 校验赛程属于该房间
                let belongs = match_schedules::table
                    .filter(match_schedules::id.eq(s.schedule_id))
                    .filter(match_schedules::room_id.eq(room_id))
                    .select(match_schedules::id)
                    .first::<i32>(txn)
                    .optional()
                    .map_err(AppError::from)?;
                if belongs.is_none() {
                    return Err(AppError::bad_request(format!(
                        "赛程 {} 不属于房间 {}",
                        s.schedule_id, room_id
                    )));
                }
                // upsert：同赛程同玩家则更新分数
                let existing: Option<i32> = match_scores::table
                    .filter(match_scores::schedule_id.eq(s.schedule_id))
                    .filter(match_scores::player_osu_id.eq(&s.player_osu_id))
                    .select(match_scores::id)
                    .first(txn)
                    .optional()
                    .map_err(AppError::from)?;
                if let Some(id) = existing {
                    diesel::update(match_scores::table.find(id))
                        .set((
                            match_scores::score.eq(s.score),
                            match_scores::updated_at.eq(now),
                        ))
                        .execute(txn)
                        .map_err(AppError::from)?;
                } else {
                    diesel::insert_into(match_scores::table)
                        .values((
                            match_scores::schedule_id.eq(s.schedule_id),
                            match_scores::player_osu_id.eq(&s.player_osu_id),
                            match_scores::player_username.eq(&s.player_username),
                            match_scores::score.eq(s.score),
                            match_scores::created_at.eq(now),
                            match_scores::updated_at.eq(now),
                        ))
                        .execute(txn)
                        .map_err(AppError::from)?;
                }
            }
            Ok(())
        })
    })
    .await?;
    Ok(ok_msg("分数保存成功", json!({ "scoresCount": count })))
}
