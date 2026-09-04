// osu! 多人房间代理路由（原 multiplayer 三端点）
//
//   GET  /api/osu/rooms?roomIds=1,2,3                          批量房间信息
//   POST /api/osu/rooms/scores                                  多房间分数（DB 优先，回退 osu API）
//   GET  /api/osu/rooms/{room_id}/playlists/{playlist_id}/scores  单 playlist 分数
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::error::{ApiResult, AppError, ok};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/osu/rooms", get(rooms_info).post(rooms_scores))
        .route(
            "/api/osu/rooms/{room_id}/playlists/{playlist_id}/scores",
            get(playlist_scores),
        )
}

/// 将 osu! API 分数对象映射为前端 DisplayScore 结构
fn display_score(
    score: &Value,
    position: usize,
    room_id: i64,
    room_name: &str,
    playlist_id: Option<i64>,
    beatmap_id: Option<i64>,
    beatmapset_id: Option<i64>,
) -> Value {
    json!({
        "userId": score["user_id"],
        "username": score["user"]["username"],
        "avatarUrl": score["user"]["avatar_url"],
        "countryCode": score["user"]["country_code"],
        "totalScore": score["total_score"],
        "accuracy": score["accuracy"],
        "maxCombo": score["max_combo"],
        "mods": score["mods"].as_array().map(|arr| {
            arr.iter().map(|m| m["acronym"].as_str().unwrap_or("").to_string()).collect::<Vec<_>>()
        }).unwrap_or_default(),
        "rank": score["rank"],
        "passed": score["passed"],
        "statistics": {
            "count300": score["statistics"]["great"],
            "count100": score["statistics"]["ok"],
            "count50": score["statistics"]["meh"],
            "countMiss": score["statistics"]["miss"],
        },
        "pp": score["pp"],
        "endedAt": score["ended_at"],
        "position": position,
        "beatmapId": beatmap_id,
        "beatmapsetId": beatmapset_id,
        "roomId": room_id,
        "roomName": room_name,
        "playlistId": playlist_id,
    })
}

fn parse_date_filter(v: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
    v.as_deref().and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.with_timezone(&chrono::Utc))
    })
}

fn apply_filters(
    score: &Value,
    start: &Option<String>,
    end: &Option<String>,
    only_valid: bool,
) -> bool {
    if start.is_some() || end.is_some() {
        let ended_at = score["ended_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
        let Some(ended_at) = ended_at else {
            return false;
        };
        if let Some(s) = parse_date_filter(start) {
            if ended_at.with_timezone(&chrono::Utc) < s {
                return false;
            }
        }
        if let Some(e) = parse_date_filter(end) {
            if ended_at.with_timezone(&chrono::Utc) > e {
                return false;
            }
        }
    }
    if only_valid && !(score["passed"].as_bool().unwrap_or(false) && score["ended_at"].is_string())
    {
        return false;
    }
    true
}

fn sort_and_rank(mut scores: Vec<Value>) -> Vec<Value> {
    scores.sort_by(|a, b| {
        b["total_score"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["total_score"].as_i64().unwrap_or(0))
    });
    for (i, s) in scores.iter_mut().enumerate() {
        s["position"] = json!(i + 1);
    }
    scores
}

// ============ GET /api/osu/rooms ============

#[derive(serde::Deserialize)]
struct RoomsQuery {
    #[serde(rename = "roomIds", alias = "ids")]
    room_ids: Option<String>,
}

async fn rooms_info(
    State(state): State<AppState>,
    Query(q): Query<RoomsQuery>,
) -> ApiResult<Json<Value>> {
    let ids_param = q
        .room_ids
        .ok_or_else(|| AppError::bad_request("房间ID列表不能为空"))?;
    let ids: Vec<String> = ids_param
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if ids.is_empty() {
        return Err(AppError::bad_request("没有有效的房间ID"));
    }
    let mut rooms = Vec::new();
    for id in &ids {
        match state.osu.get_json(&format!("/rooms/{id}"), &[]).await {
            Ok(room) => rooms.push(json!({
                "id": id.parse::<i64>().unwrap_or(0),
                "name": room["name"],
                "category": room["category"],
                "type": room["type"],
                "startsAt": room["starts_at"],
                "endsAt": room["ends_at"],
                "participantCount": room["participant_count"],
                "host": room["host"],
                "playlistCount": room["playlist"].as_array().map(|a| a.len()).unwrap_or(0),
            })),
            Err(e) => tracing::warn!("获取房间 {id} 失败: {}", e.message),
        }
    }
    Ok(ok(json!(rooms)))
}

// ============ POST /api/osu/rooms/scores ============

#[derive(serde::Deserialize)]
struct RoomScoresBody {
    #[serde(rename = "roomIds")]
    room_ids: Vec<i64>,
    #[serde(rename = "startDate", default)]
    start_date: Option<String>,
    #[serde(rename = "endDate", default)]
    end_date: Option<String>,
    #[serde(rename = "onlyValid", default)]
    only_valid: bool,
}

async fn rooms_scores(
    State(state): State<AppState>,
    Json(body): Json<RoomScoresBody>,
) -> ApiResult<Json<Value>> {
    if body.room_ids.is_empty() {
        return Err(AppError::bad_request("房间ID数组不能为空"));
    }

    // 第一步：优先从数据库获取
    use crate::models::{MatchRoom, MatchScore};
    use crate::schema::{match_rooms, match_schedules, match_scores};
    let mut all_scores: Vec<Value> = Vec::new();
    let mut rooms_data: Vec<Value> = Vec::new();
    let mut db_room_ids: Vec<i32> = Vec::new();
    for rid in &body.room_ids {
        let rid_i = i32::try_from(*rid).unwrap_or(-1);
        let result = crate::db::run_db(&state.pool, move |conn| {
            let room = match_rooms::table
                .find(rid_i)
                .first::<MatchRoom>(conn)
                .optional()
                .map_err(crate::error::AppError::from)?;
            let schedule_ids: Vec<i32> = match_schedules::table
                .filter(match_schedules::room_id.eq(rid_i))
                .select(match_schedules::id)
                .load(conn)
                .map_err(crate::error::AppError::from)?;
            let scores = if schedule_ids.is_empty() {
                vec![]
            } else {
                match_scores::table
                    .filter(match_scores::schedule_id.eq_any(&schedule_ids))
                    .load::<MatchScore>(conn)
                    .map_err(crate::error::AppError::from)?
            };
            Ok((room, scores))
        })
        .await?;
        let (room, scores) = result;
        if !scores.is_empty() {
            db_room_ids.push(rid_i);
            let room_name = room
                .as_ref()
                .map(|r| r.room_name.clone())
                .unwrap_or_else(|| format!("Room {rid}"));
            rooms_data.push(json!({
                "id": rid,
                "name": room_name,
                "category": "normal",
                "participantCount": scores.len(),
                "active": false,
                "playlistCount": scores.len(),
            }));
            for s in &scores {
                let score = json!({
                    "userId": s.player_osu_id.parse::<i64>().unwrap_or(0),
                    "username": s.player_username,
                    "totalScore": s.score,
                    "passed": true,
                    "endedAt": s.updated_at.and_utc().to_rfc3339(),
                });
                if !apply_filters(&score, &body.start_date, &body.end_date, body.only_valid) {
                    continue;
                }
                let mut ds = display_score(&score, 1, *rid, &room_name, None, None, None);
                ds["user_id"] = json!(s.player_osu_id);
                all_scores.push(ds);
            }
        }
    }
    if !db_room_ids.is_empty() {
        let sorted = sort_and_rank(all_scores);
        return Ok(ok(json!({
            "scores": sorted,
            "rooms": rooms_data,
            "source": "database",
        })));
    }

    // 第二步：回退 osu API
    for rid in &body.room_ids {
        let room = match state.osu.get_json(&format!("/rooms/{rid}"), &[]).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("获取房间 {rid} 失败: {}", e.message);
                continue;
            }
        };
        let room_name = room["name"].as_str().unwrap_or("Room").to_string();
        rooms_data.push(json!({
            "id": rid,
            "name": room_name,
            "category": room["category"],
            "participantCount": room["participant_count"],
            "active": room["active"],
            "playlistCount": room["playlist"].as_array().map(|a| a.len()).unwrap_or(0),
        }));
        let empty = Vec::new();
        let playlists = room["playlist"].as_array().unwrap_or(&empty);
        for playlist in playlists {
            let playlist_id = playlist["id"].as_i64();
            // playlist 详情（获取 beatmap 信息）
            let mut beatmap_id = playlist["beatmap_id"].as_i64();
            let mut beatmapset_id = playlist["beatmap"]["beatmapset_id"].as_i64();
            if let Some(pid) = playlist_id {
                if let Ok(detail) = state
                    .osu
                    .get_json(&format!("/rooms/{rid}/playlist/{pid}"), &[])
                    .await
                {
                    beatmap_id = detail["beatmap_id"].as_i64();
                    beatmapset_id = detail["beatmap"]["beatmapset_id"].as_i64();
                }
            }
            // 分数
            let scores_path = match playlist_id {
                Some(pid) => format!("/rooms/{rid}/playlist/{pid}/scores"),
                None => continue,
            };
            match state
                .osu
                .get_json(&scores_path, &[("limit", "100"), ("sort", "score_desc")])
                .await
            {
                Ok(scores_data) => {
                    let scores_list = scores_data["scores"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    for (idx, score) in scores_list.iter().enumerate() {
                        let ds = display_score(
                            score,
                            idx + 1,
                            *rid,
                            &room_name,
                            playlist_id,
                            beatmap_id,
                            beatmapset_id,
                        );
                        if !apply_filters(&ds, &body.start_date, &body.end_date, body.only_valid) {
                            continue;
                        }
                        all_scores.push(ds);
                    }
                }
                Err(e) => {
                    tracing::warn!("获取房间 {rid} playlist 分数失败: {}", e.message);
                    continue;
                }
            }
        }
    }
    let sorted = sort_and_rank(all_scores);
    Ok(ok(json!({
        "scores": sorted,
        "rooms": rooms_data,
    })))
}

// ============ GET /api/osu/rooms/{rid}/playlists/{pid}/scores ============

async fn playlist_scores(
    State(state): State<AppState>,
    Path((room_id, playlist_id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    // 房间名与 beatmap 信息
    let mut room_name = String::new();
    if let Ok(room) = state.osu.get_json(&format!("/rooms/{room_id}"), &[]).await {
        room_name = room["name"].as_str().unwrap_or("").to_string();
    }
    let mut beatmap_id: Option<i64> = None;
    let mut beatmapset_id: Option<i64> = None;
    if let Ok(playlist) = state
        .osu
        .get_json(&format!("/rooms/{room_id}/playlist/{playlist_id}"), &[])
        .await
    {
        beatmap_id = playlist["beatmap_id"].as_i64();
        beatmapset_id = playlist["beatmap"]["beatmapset_id"].as_i64();
    }
    let scores_data = state
        .osu
        .get_json(
            &format!("/rooms/{room_id}/playlist/{playlist_id}/scores"),
            &[("limit", "100"), ("sort", "score_desc")],
        )
        .await?;
    let rid_num = room_id.parse::<i64>().unwrap_or(0);
    let pid_num = playlist_id.parse::<i64>().unwrap_or(0);
    let scores_list = scores_data["scores"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let display: Vec<Value> = scores_list
        .iter()
        .enumerate()
        .map(|(idx, score)| {
            display_score(
                score,
                idx + 1,
                rid_num,
                &room_name,
                Some(pid_num),
                beatmap_id,
                beatmapset_id,
            )
        })
        .collect();
    Ok(ok(json!(display)))
}
