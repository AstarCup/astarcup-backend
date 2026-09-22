// 工作人员路由（合并原 staff-room-assignments 与 staff/stats）
//
//   GET    /api/staff/assignments?roomId=   分配列表（管理员）
//   POST   /api/staff/assignments           申请加入房间（staff_osuId 本身须为管理员）
//   DELETE /api/staff/assignments/{id}      撤销分配
//   GET    /api/staff/stats                 管理面板统计
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::db::run_db;
use crate::enums::StaffRole;
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::{MapSelection, MatchRoom, MatchSchedule, StaffRoomAssignment, User};
use crate::schema::{
    map_selections, match_rooms, match_schedules, staff_room_assignments, tournament_settings,
    users,
};
use crate::state::AppState;
use crate::util::storage_path;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/staff/assignments",
            get(list_assignments).post(create_assignment),
        )
        .route(
            "/api/staff/assignments/{id}",
            axum::routing::delete(delete_assignment),
        )
        .route("/api/staff/stats", get(stats))
}

// ============ GET /api/staff/assignments ============

#[derive(serde::Deserialize)]
struct AssignmentsQuery {
    #[serde(rename = "roomId")]
    room_id: Option<i32>,
}

async fn list_assignments(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Query(q): Query<AssignmentsQuery>,
) -> ApiResult<Json<Value>> {
    let (assignments, rooms) = run_db(&state.pool, move |conn| {
        let mut query = staff_room_assignments::table.into_boxed();
        if let Some(rid) = q.room_id {
            query = query.filter(staff_room_assignments::room_id.eq(rid));
        }
        let assignments = query
            .order(staff_room_assignments::created_at.desc())
            .load::<StaffRoomAssignment>(conn)
            .map_err(AppError::from)?;
        let rooms = match_rooms::table
            .load::<MatchRoom>(conn)
            .map_err(AppError::from)?;
        Ok((assignments, rooms))
    })
    .await?;
    let room_by_id: std::collections::HashMap<i32, Value> =
        rooms.into_iter().map(|r| (r.id, r.to_json())).collect();
    let list: Vec<Value> = assignments
        .iter()
        .map(|a| a.to_json(room_by_id.get(&a.room_id)))
        .collect();
    Ok(ok(json!(list)))
}

// ============ POST /api/staff/assignments ============

#[derive(serde::Deserialize)]
struct CreateAssignmentBody {
    #[serde(rename = "roomId", alias = "room_id")]
    room_id: i64,
    #[serde(rename = "staffOsuId", alias = "staff_osuId")]
    staff_osu_id: String,
    #[serde(rename = "staffUsername", alias = "staff_username")]
    staff_username: String,
    #[serde(rename = "staffRole", alias = "staff_role")]
    staff_role: String,
}

async fn create_assignment(
    State(state): State<AppState>,
    Json(body): Json<CreateAssignmentBody>,
) -> ApiResult<Json<Value>> {
    let role = StaffRole::parse(&body.staff_role)
        .ok_or_else(|| AppError::bad_request("无效的工作人员角色"))?;
    let room_id = i32::try_from(body.room_id).map_err(|_| AppError::bad_request("无效的房间ID"))?;

    // 校验申请者（staff_osuId）本身是管理员
    let staff_id = body.staff_osu_id.clone();
    let is_admin = run_db(&state.pool, move |conn| {
        let group: Option<crate::enums::UserGroup> = users::table
            .filter(users::osu_id.eq(&staff_id))
            .select(users::user_group)
            .first(conn)
            .optional()
            .map_err(AppError::from)?;
        Ok(group == Some(crate::enums::UserGroup::Admin))
    })
    .await?;
    if !is_admin {
        return Err(AppError::forbidden("没有权限申请staff角色"));
    }

    // 防止重复申请同房间同角色
    let staff_id_c = body.staff_osu_id.clone();
    let assignment = run_db(&state.pool, move |conn| {
        let exists = staff_room_assignments::table
            .filter(staff_room_assignments::room_id.eq(room_id))
            .filter(staff_room_assignments::staff_osu_id.eq(&staff_id_c))
            .filter(staff_room_assignments::role.eq(role))
            .first::<StaffRoomAssignment>(conn)
            .optional()
            .map_err(AppError::from)?;
        if exists.is_some() {
            return Err(AppError::bad_request("已经申请过这个角色的分配了"));
        }
        let now = Utc::now().naive_utc();
        diesel::insert_into(staff_room_assignments::table)
            .values((
                staff_room_assignments::room_id.eq(room_id),
                staff_room_assignments::staff_osu_id.eq(&body.staff_osu_id),
                staff_room_assignments::staff_username.eq(&body.staff_username),
                staff_room_assignments::role.eq(role),
                staff_room_assignments::created_at.eq(now),
                staff_room_assignments::updated_at.eq(now),
            ))
            .get_result::<StaffRoomAssignment>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok_msg("已成功加入房间", assignment.to_json(None)))
}

// ============ DELETE /api/staff/assignments/{id} ============

async fn delete_assignment(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    let n = run_db(&state.pool, move |conn| {
        diesel::delete(staff_room_assignments::table.find(id))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("撤销失败，分配记录不存在"));
    }
    Ok(ok_msg("已撤销房间分配", Value::Null))
}

// ============ GET /api/staff/stats ============

async fn stats(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
) -> ApiResult<Json<Value>> {
    let (mut data, seasons) = run_db(&state.pool, move |conn| {
        let all_users = users::table.load::<User>(conn).map_err(AppError::from)?;
        let schedules = match_schedules::table
            .load::<MatchSchedule>(conn)
            .map_err(AppError::from)?;
        let rooms = match_rooms::table
            .load::<MatchRoom>(conn)
            .map_err(AppError::from)?;
        let maps = map_selections::table
            .load::<MapSelection>(conn)
            .map_err(AppError::from)?;
        let settings = tournament_settings::table
            .load::<crate::models::TournamentSetting>(conn)
            .map_err(AppError::from)?;

        let approved_players = all_users
            .iter()
            .filter(|u| u.registration_status == crate::enums::RegistrationStatus::Approved)
            .count() as i64;
        let admin_count = all_users
            .iter()
            .filter(|u| u.user_group == crate::enums::UserGroup::Admin)
            .count() as i64;
        let replay_count = schedules.iter().filter(|s| s.replay_link.is_some()).count() as i64;

        let mut settings_map = std::collections::HashMap::new();
        for s in &settings {
            settings_map.insert(s.setting_key.clone(), s.setting_value.clone());
        }
        let get = |k: &str| settings_map.get(k).cloned().unwrap_or_default();
        let or_default = |v: String, d: &str| if v.is_empty() { d.to_string() } else { v };
        let tournament_settings_display = json!({
            "currentSeason": or_default(get("current_season"), "s1"),
            "currentSeasonStage": or_default(get("current_season_stage"), "registration"),
            "minPp": or_default(get("min_pp_for_registration"), "0"),
            "maxPp": or_default(get("max_pp_for_registration"), "7000"),
            "registrationEnabled": get("registration_enabled") != "false",
            "mappoolVisible": get("mappool_visible") == "true",
            "adminCount": admin_count,
        });

        // 按 season × category 聚合 mappool 统计
        let approved_maps: Vec<&MapSelection> = maps.iter().filter(|m| m.approved).collect();
        let testing_maps: Vec<&MapSelection> =
            maps.iter().filter(|m| !m.approved && m.padding).collect();
        let pending_maps: Vec<&MapSelection> =
            maps.iter().filter(|m| !m.approved && !m.padding).collect();

        // 赛季列表 = 选图赛季 ∪ 选手赛季
        let mut seasons: Vec<String> = maps.iter().filter_map(|m| m.season.clone()).collect();
        for u in &all_users {
            if let Some(ss) = &u.season {
                seasons.extend(ss.iter().cloned());
            }
        }
        seasons.sort();
        seasons.dedup();

        let categories = [
            "qualification",
            "ro16",
            "quarterfinals",
            "semifinals",
            "finals",
            "grandfinals",
        ];
        let labels = ["QUA", "RO16", "QF", "SF", "F", "GF"];

        let mut season_stats = serde_json::Map::new();
        for season in &seasons {
            let mut stats_arr = Vec::new();
            for (cat, label) in categories.iter().zip(labels.iter()) {
                let matches = |m: &&MapSelection| {
                    m.season.as_deref() == Some(season.as_str())
                        && m.category.as_deref() == Some(*cat)
                };
                let a: Vec<&MapSelection> = approved_maps.iter().copied().filter(matches).collect();
                let t: Vec<&MapSelection> = testing_maps.iter().copied().filter(matches).collect();
                let p: Vec<&MapSelection> = pending_maps.iter().copied().filter(matches).collect();
                if a.is_empty() && t.is_empty() && p.is_empty() {
                    continue;
                }
                let avg = |list: &[&MapSelection], f: fn(&MapSelection) -> f64| -> f64 {
                    if list.is_empty() {
                        0.0
                    } else {
                        list.iter().map(|m| f(m)).sum::<f64>() / list.len() as f64
                    }
                };
                let avg_len = |list: &[&MapSelection]| -> i64 {
                    if list.is_empty() {
                        0
                    } else {
                        (list.iter().map(|m| m.total_length as i64).sum::<i64>() as f64
                            / list.len() as f64)
                            .round() as i64
                    }
                };
                let sr2 = |v: f64| -> f64 { (v * 100.0).round() / 100.0 };
                let round_i64 = |v: f64| -> i64 { v.round() as i64 };
                stats_arr.push(json!({
                    "category": cat,
                    "label": label,
                    "avgStarRating": sr2(avg(&a, |m| m.star_rating)),
                    "avgStarRatingTesting": sr2(avg(&t, |m| m.star_rating)),
                    "avgStarRatingPending": sr2(avg(&p, |m| m.star_rating)),
                    "avgLength": avg_len(&a),
                    "avgLengthTesting": avg_len(&t),
                    "avgLengthPending": avg_len(&p),
                    "avgAr": sr2(avg(&a, |m| m.ar)),
                    "avgArTesting": sr2(avg(&t, |m| m.ar)),
                    "avgArPending": sr2(avg(&p, |m| m.ar)),
                    "avgCs": sr2(avg(&a, |m| m.cs)),
                    "avgCsTesting": sr2(avg(&t, |m| m.cs)),
                    "avgCsPending": sr2(avg(&p, |m| m.cs)),
                    "avgOd": sr2(avg(&a, |m| m.od)),
                    "avgOdTesting": sr2(avg(&t, |m| m.od)),
                    "avgOdPending": sr2(avg(&p, |m| m.od)),
                    "avgHp": sr2(avg(&a, |m| m.hp)),
                    "avgHpTesting": sr2(avg(&t, |m| m.hp)),
                    "avgHpPending": sr2(avg(&p, |m| m.hp)),
                    "avgBpm": sr2(avg(&a, |m| m.bpm)),
                    "avgBpmTesting": sr2(avg(&t, |m| m.bpm)),
                    "avgBpmPending": sr2(avg(&p, |m| m.bpm)),
                    "avgMaxCombo": round_i64(avg(&a, |m| m.max_combo as f64)),
                    "avgMaxComboTesting": round_i64(avg(&t, |m| m.max_combo as f64)),
                    "avgMaxComboPending": round_i64(avg(&p, |m| m.max_combo as f64)),
                    "count": a.len(),
                    "testingCount": t.len(),
                    "pendingCount": p.len(),
                }));
            }
            if !stats_arr.is_empty() {
                season_stats.insert(season.clone(), Value::Array(stats_arr));
            }
        }

        // 每赛季选手统计（season 数组包含该赛季的用户）
        let mut season_player_stats = serde_json::Map::new();
        for season in &seasons {
            let in_season: Vec<&User> = all_users
                .iter()
                .filter(|u| {
                    u.season
                        .as_ref()
                        .map(|ss| ss.iter().any(|s| s == season))
                        .unwrap_or(false)
                })
                .collect();
            let approved = in_season
                .iter()
                .filter(|u| u.registration_status == crate::enums::RegistrationStatus::Approved)
                .count() as i64;
            season_player_stats.insert(
                season.clone(),
                json!({
                    "totalRegistrations": in_season.len() as i64,
                    "approvedPlayers": approved,
                    "unapprovedPlayers": in_season.len() as i64 - approved,
                }),
            );
        }

        Ok((
            json!({
                "approvedPlayers": approved_players,
                "matchRooms": rooms.len(),
                "mappoolSeasons": seasons.len(),
                "replayCount": replay_count,
                "totalRegistrations": all_users.len(),
                "totalSchedules": schedules.len(),
                "seasonMappoolStats": Value::Object(season_stats),
                "seasonStats": Value::Object(season_player_stats),
                "tournamentSettings": tournament_settings_display,
            }),
            seasons,
        ))
    })
    .await?;

    // 每赛季回放数：扫描对应存储目录下的 .osr 文件
    if let Some(map) = data["seasonStats"].as_object_mut() {
        for season in &seasons {
            let cnt = season_replay_count(&state, season).await?;
            if let Some(v) = map.get_mut(season) {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("replayCount".into(), json!(cnt));
                }
            }
        }
    }

    Ok(ok(data))
}

/// 统计某赛季存储目录下所有 .osr 回放文件数量（递归）
async fn season_replay_count(state: &AppState, season: &str) -> Result<i64, AppError> {
    let base = storage_path(&state.config, season)?;
    if !base.exists() {
        return Ok(0);
    }
    let mut count = 0i64;
    let mut dirs = vec![base];
    while let Some(dir) = dirs.pop() {
        let mut entries = tokio::fs::read_dir(&dir)
            .await
            .map_err(|e| AppError::internal(format!("读取目录失败: {e}")))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| AppError::internal(format!("读取目录失败: {e}")))?
        {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("osr") {
                count += 1;
            }
        }
    }
    Ok(count)
}
