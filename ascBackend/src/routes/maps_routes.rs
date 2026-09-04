// 选图系统路由（合并原 map-selections、bulk-approve、map-ratings、batch-comments）
//
//   GET    /api/map-selections?season=&category=&approved=&padding=   选图列表（approved=true 公开）
//   POST   /api/map-selections                                        添加选图（URL 或自定图池）
//   PATCH  /api/map-selections/{id}                                   更新选图
//   DELETE /api/map-selections/{id}                                   删除选图
//   POST   /api/map-selections/bulk-approve                           批量过审
//   GET    /api/map-selections/comments?ids=1,2,3 或 ?mapSelectionId=  批量/单个评论
//   POST   /api/map-selections/{id}/comments                          添加评论
//   DELETE /api/map-selections/comments/{id}                          删除评论（管理员或本人）
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::db::run_db;
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::{MapComment, MapSelection};
use crate::schema::{map_comments, map_selections, users};
use crate::state::AppState;
use crate::util::parse_beatmap_url;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/map-selections",
            get(list_selections).post(add_selection),
        )
        .route("/api/map-selections/bulk-approve", post(bulk_approve))
        .route("/api/map-selections/comments", get(get_comments))
        .route(
            "/api/map-selections/comments/{id}",
            axum::routing::delete(delete_comment),
        )
        .route(
            "/api/map-selections/{id}",
            axum::routing::patch(update_selection).delete(delete_selection),
        )
        .route("/api/map-selections/{id}/comments", post(add_comment))
}

// ============ 工具 ============

/// 批量取出选图者用户名/头像
async fn user_map(
    state: &AppState,
    osu_ids: Vec<String>,
) -> Result<std::collections::HashMap<String, (String, String)>, AppError> {
    if osu_ids.is_empty() {
        return Ok(Default::default());
    }
    run_db(&state.pool, move |conn| {
        users::table
            .filter(users::osu_id.eq_any(osu_ids))
            .select((users::osu_id, users::username, users::avatar_url))
            .load::<(String, String, Option<String>)>(conn)
            .map(|rows| {
                rows.into_iter()
                    .map(|(id, name, avatar)| (id, (name, avatar.unwrap_or_default())))
                    .collect()
            })
            .map_err(AppError::from)
    })
    .await
}

fn selection_to_json(m: &MapSelection, u: Option<&(String, String)>) -> Value {
    m.to_json(u.map(|(n, a)| (n.as_str(), a.as_str())))
}

// ============ GET /api/map-selections ============

#[derive(serde::Deserialize)]
struct SelectionsQuery {
    season: Option<String>,
    category: Option<String>,
    approved: Option<String>,
    padding: Option<String>,
}

async fn list_selections(
    State(state): State<AppState>,
    Query(q): Query<SelectionsQuery>,
    session: Option<crate::auth::SessionUser>,
) -> ApiResult<Json<Value>> {
    let approved_only = q.approved.as_deref() == Some("true");
    if !approved_only && session.is_none() {
        return Err(AppError::bad_request("缺少osu ID"));
    }
    if !approved_only {
        // 非公开访问需要管理员权限（原版 verifyMapSelectionAuth/verifyReplayAuth 均为管理员）
        let permissions = crate::routes::users_routes::permissions_json(
            &state,
            &session.as_ref().unwrap().osu_id,
        )
        .await?;
        if !permissions["isadmin"].as_bool().unwrap_or(false) {
            return Err(AppError::forbidden("您没有权限访问选图系统"));
        }
    }

    let season = q.season.unwrap_or_else(|| "s1".into());
    let category = q.category.filter(|c| c != "all");
    let padding = q.padding.as_deref().map(|p| p == "true");

    let rows = run_db(&state.pool, move |conn| {
        let mut query = map_selections::table
            .filter(map_selections::season.eq(&season))
            .into_boxed();
        if let Some(c) = &category {
            query = query.filter(map_selections::category.eq(c));
        }
        if let Some(p) = padding {
            query = query.filter(map_selections::padding.eq(p));
        }
        query
            .order(map_selections::selected_at.desc())
            .load::<MapSelection>(conn)
            .map_err(AppError::from)
    })
    .await?;

    let rows = if approved_only {
        rows.into_iter().filter(|m| m.approved).collect::<Vec<_>>()
    } else {
        rows
    };
    let user_ids: Vec<String> = rows.iter().map(|m| m.selected_by.clone()).collect();
    let umap = user_map(&state, user_ids).await?;

    let selections: Vec<Value> = rows
        .iter()
        .map(|m| selection_to_json(m, umap.get(&m.selected_by)))
        .collect();
    Ok(ok(json!(selections)))
}

// ============ POST /api/map-selections ============

#[derive(serde::Deserialize)]
struct AddSelectionBody {
    url: Option<String>,
    #[serde(rename = "selectedMods", default)]
    selected_mods: Option<String>,
    #[serde(rename = "modPosition", default)]
    mod_position: Option<i64>,
    #[serde(rename = "customDTRate", default)]
    custom_dt_rate: Option<f64>,
    #[serde(rename = "customModName", default)]
    custom_mod_name: Option<String>,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    approved: Option<bool>,
    #[serde(rename = "selectedBy")]
    selected_by: String,
    #[serde(rename = "season", default)]
    season: Option<String>,
    #[serde(rename = "category", default)]
    category: Option<String>,
    #[serde(rename = "moddedStats", default)]
    modded_stats: Option<Value>,
    // 自定图池
    #[serde(rename = "isCustomPool", alias = "is_custom_pool", default)]
    is_custom_pool: bool,
    #[serde(rename = "beatmapId", alias = "beatmap_id", default)]
    beatmap_id: Option<i64>,
    #[serde(rename = "beatmapsetId", alias = "beatmapset_id", default)]
    beatmapset_id: Option<i64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(rename = "title_unicode", default)]
    title_unicode: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(rename = "artist_unicode", default)]
    artist_unicode: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    creator: Option<String>,
    #[serde(rename = "starRating", default)]
    star_rating: Option<f64>,
    #[serde(default)]
    bpm: Option<f64>,
    #[serde(rename = "totalLength", default)]
    total_length: Option<i64>,
    #[serde(rename = "maxCombo", default)]
    max_combo: Option<i64>,
    #[serde(default)]
    ar: Option<f64>,
    #[serde(default)]
    cs: Option<f64>,
    #[serde(default)]
    od: Option<f64>,
    #[serde(default)]
    hp: Option<f64>,
    #[serde(rename = "coverUrl", default)]
    cover_url: Option<String>,
}

async fn add_selection(
    State(state): State<AppState>,
    Json(body): Json<AddSelectionBody>,
) -> ApiResult<Json<Value>> {
    if body.selected_by.is_empty() {
        return Err(AppError::bad_request("缺少必要参数：selectedBy"));
    }
    let permissions =
        crate::routes::users_routes::permissions_json(&state, &body.selected_by).await?;
    if !permissions["isadmin"].as_bool().unwrap_or(false) {
        return Err(AppError::forbidden("您没有权限访问选图系统"));
    }

    let now = Utc::now().naive_utc();
    let encoded_mods = MapSelection::encode_mods(
        body.selected_mods.as_deref().unwrap_or("NM"),
        body.custom_dt_rate,
        body.custom_mod_name.as_deref(),
    );

    let data: Value;
    if body.is_custom_pool {
        // 自定图池（手动输入）
        let (title, artist, version, creator) =
            match (&body.title, &body.artist, &body.version, &body.creator) {
                (Some(t), Some(a), Some(v), Some(c)) => (t, a, v, c),
                _ => {
                    return Err(AppError::bad_request(
                        "自定图池缺少必要参数：title, artist, version, creator",
                    ));
                }
            };
        let final_bid = body
            .beatmap_id
            .unwrap_or_else(|| -chrono::Utc::now().timestamp());
        let final_bsid = body
            .beatmapset_id
            .unwrap_or_else(|| -chrono::Utc::now().timestamp() - 1);
        data = json!({
            "beatmapId": final_bid,
            "beatmapsetId": final_bsid,
            "title": title,
            "title_unicode": body.title_unicode.clone().unwrap_or_else(|| title.clone()),
            "artist": artist,
            "artist_unicode": body.artist_unicode.clone().unwrap_or_else(|| artist.clone()),
            "version": version,
            "creator": creator,
            "starRating": body.star_rating.unwrap_or(5.0),
            "bpm": body.bpm.unwrap_or(180.0),
            "totalLength": body.total_length.unwrap_or(120),
            "maxCombo": body.max_combo.unwrap_or(1000),
            "ar": body.ar.unwrap_or(9.0),
            "cs": body.cs.unwrap_or(4.0),
            "od": body.od.unwrap_or(8.0),
            "hp": body.hp.unwrap_or(6.0),
            "selectedMods": encoded_mods,
            "modPosition": body.mod_position.unwrap_or(1),
            "comment": body.comment.clone().unwrap_or_default(),
            "selectedBy": body.selected_by,
            "season": body.season.clone().unwrap_or_else(|| "s1".into()),
            "category": body.category.clone().unwrap_or_else(|| "qualification".into()),
            "url": body.url.clone().unwrap_or_else(|| format!("custom://pool/{final_bid}")),
            "coverUrl": body.cover_url.clone().unwrap_or_default(),
            "approved": body.approved.unwrap_or(false),
            "padding": false,
        });
    } else {
        // 普通 URL 选图
        let url = body
            .url
            .as_deref()
            .ok_or_else(|| AppError::bad_request("缺少必要参数：url"))?;
        let parsed = parse_beatmap_url(url);
        if parsed.beatmap_id.is_none() && parsed.beatmapset_id.is_none() {
            return Err(AppError::bad_request("无效的osu! beatmap URL"));
        }

        let beatmap = if let Some(bid) = parsed.beatmap_id {
            fetch_beatmap_info(&state, bid).await?
        } else {
            let bsid = parsed.beatmapset_id.unwrap();
            let maps = fetch_beatmapset_info(&state, bsid).await?;
            maps.first()
                .cloned()
                .ok_or_else(|| AppError::bad_request("该beatmapset中没有找到任何beatmap"))?
        };

        // 使用 mod 计算后的参数（如提供）或原参数
        let (ar, cs, od, hp, sr, bpm, max_combo) = if let Some(m) = &body.modded_stats {
            (
                m.get("ar")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(beatmap["ar"].as_f64().unwrap_or(0.0)),
                m.get("cs")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(beatmap["cs"].as_f64().unwrap_or(0.0)),
                m.get("od")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(beatmap["od"].as_f64().unwrap_or(0.0)),
                m.get("hp")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(beatmap["hp"].as_f64().unwrap_or(0.0)),
                m.get("star_rating")
                    .or_else(|| m.get("starRating"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(beatmap["star_rating"].as_f64().unwrap_or(0.0)),
                m.get("bpm")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(beatmap["bpm"].as_f64().unwrap_or(0.0)),
                m.get("max_combo")
                    .or_else(|| m.get("maxCombo"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
            )
        } else {
            (
                beatmap["ar"].as_f64().unwrap_or(0.0),
                beatmap["cs"].as_f64().unwrap_or(0.0),
                beatmap["od"].as_f64().unwrap_or(0.0),
                beatmap["hp"].as_f64().unwrap_or(0.0),
                beatmap["star_rating"].as_f64().unwrap_or(0.0),
                beatmap["bpm"].as_f64().unwrap_or(0.0),
                beatmap["max_combo"].as_i64().unwrap_or(0),
            )
        };

        data = json!({
            "beatmapId": beatmap["id"],
            "beatmapsetId": beatmap["beatmapset_id"],
            "title": beatmap["title"],
            "title_unicode": beatmap["title_unicode"],
            "artist": beatmap["artist"],
            "artist_unicode": beatmap["artist_unicode"],
            "version": beatmap["version"],
            "creator": beatmap["creator"],
            "starRating": sr,
            "bpm": bpm,
            "totalLength": beatmap["total_length"],
            "maxCombo": max_combo,
            "ar": ar,
            "cs": cs,
            "od": od,
            "hp": hp,
            "selectedMods": encoded_mods,
            "modPosition": body.mod_position.unwrap_or(1),
            "comment": body.comment.clone().unwrap_or_default(),
            "selectedBy": body.selected_by,
            "season": body.season.clone().unwrap_or_else(|| "s1".into()),
            "category": body.category.clone().unwrap_or_else(|| "qualification".into()),
            "url": beatmap["url"],
            "coverUrl": beatmap["cover_url"].as_str().unwrap_or(""),
            "approved": body.approved.unwrap_or(false),
            "padding": false,
        });
    }

    let insert_data = data.clone();
    let inserted = run_db(&state.pool, move |conn| {
        diesel::insert_into(map_selections::table)
            .values((
                map_selections::beatmap_id
                    .eq(insert_data["beatmapId"].as_i64().unwrap_or(0) as i32),
                map_selections::beatmapset_id
                    .eq(insert_data["beatmapsetId"].as_i64().unwrap_or(0) as i32),
                map_selections::title.eq(insert_data["title"].as_str().unwrap_or("").to_string()),
                map_selections::title_unicode
                    .eq(insert_data["title_unicode"].as_str().map(|s| s.to_string())),
                map_selections::artist.eq(insert_data["artist"].as_str().unwrap_or("").to_string()),
                map_selections::artist_unicode.eq(insert_data["artist_unicode"]
                    .as_str()
                    .map(|s| s.to_string())),
                map_selections::version
                    .eq(insert_data["version"].as_str().unwrap_or("").to_string()),
                map_selections::creator
                    .eq(insert_data["creator"].as_str().unwrap_or("").to_string()),
                map_selections::star_rating.eq(insert_data["starRating"].as_f64().unwrap_or(0.0)),
                map_selections::bpm.eq(insert_data["bpm"].as_f64().unwrap_or(0.0)),
                map_selections::total_length
                    .eq(insert_data["totalLength"].as_i64().unwrap_or(0) as i32),
                map_selections::max_combo.eq(insert_data["maxCombo"].as_i64().unwrap_or(0) as i32),
                map_selections::ar.eq(insert_data["ar"].as_f64().unwrap_or(0.0)),
                map_selections::cs.eq(insert_data["cs"].as_f64().unwrap_or(0.0)),
                map_selections::od.eq(insert_data["od"].as_f64().unwrap_or(0.0)),
                map_selections::hp.eq(insert_data["hp"].as_f64().unwrap_or(0.0)),
                map_selections::selected_mods.eq(Some(
                    insert_data["selectedMods"]
                        .as_str()
                        .unwrap_or("NM")
                        .to_string(),
                )),
                map_selections::mod_position
                    .eq(Some(insert_data["modPosition"].as_i64().unwrap_or(1) as i32)),
                map_selections::comment.eq(Some(
                    insert_data["comment"].as_str().unwrap_or("").to_string(),
                )),
                map_selections::selected_by
                    .eq(insert_data["selectedBy"].as_str().unwrap_or("").to_string()),
                map_selections::selected_at.eq(now),
                map_selections::season.eq(Some(
                    insert_data["season"].as_str().unwrap_or("s1").to_string(),
                )),
                map_selections::category.eq(Some(
                    insert_data["category"]
                        .as_str()
                        .unwrap_or("qualification")
                        .to_string(),
                )),
                map_selections::url.eq(insert_data["url"].as_str().map(|s| s.to_string())),
                map_selections::cover_url
                    .eq(insert_data["coverUrl"].as_str().map(|s| s.to_string())),
                map_selections::approved.eq(insert_data["approved"].as_bool().unwrap_or(false)),
                map_selections::padding.eq(insert_data["padding"].as_bool().unwrap_or(false)),
                map_selections::is_custome.eq(false),
                map_selections::is_origin.eq(false),
                map_selections::created_at.eq(now),
                map_selections::updated_at.eq(now),
            ))
            .returning(map_selections::id)
            .get_result::<i32>(conn)
            .map_err(AppError::from)
    })
    .await?;

    let message = if body.is_custom_pool {
        "自定图池添加成功"
    } else {
        "选图添加成功"
    };
    let mut selection = data;
    selection["id"] = json!(inserted);
    Ok(ok_msg(message, selection))
}

// ============ PATCH /api/map-selections/{id} ============

#[derive(serde::Deserialize)]
struct UpdateSelectionBody {
    #[serde(rename = "selectedMods", default)]
    selected_mods: Option<String>,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    approved: Option<bool>,
    #[serde(default)]
    padding: Option<bool>,
    #[serde(default)]
    title: Option<String>,
    #[serde(rename = "title_unicode", default)]
    title_unicode: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(rename = "artist_unicode", default)]
    artist_unicode: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(rename = "totalLength", default)]
    total_length: Option<i64>,
    #[serde(rename = "maxCombo", default)]
    max_combo: Option<i64>,
    #[serde(rename = "customModName", default)]
    custom_mod_name: Option<String>,
    #[serde(rename = "customDTRate", default)]
    custom_dt_rate: Option<f64>,
    #[serde(rename = "starRating", default)]
    star_rating: Option<f64>,
    #[serde(rename = "moddedStats", default)]
    modded_stats: Option<Value>,
    #[serde(default)]
    ar: Option<f64>,
    #[serde(default)]
    cs: Option<f64>,
    #[serde(default)]
    od: Option<f64>,
    #[serde(default)]
    hp: Option<f64>,
    #[serde(default)]
    bpm: Option<f64>,
}

async fn update_selection(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(id): Path<i32>,
    Json(body): Json<UpdateSelectionBody>,
) -> ApiResult<Json<Value>> {
    let any_update = body.selected_mods.is_some()
        || body.comment.is_some()
        || body.approved.is_some()
        || body.padding.is_some()
        || body.title.is_some()
        || body.title_unicode.is_some()
        || body.artist.is_some()
        || body.artist_unicode.is_some()
        || body.version.is_some()
        || body.category.is_some()
        || body.total_length.is_some()
        || body.max_combo.is_some()
        || body.custom_mod_name.is_some()
        || body.custom_dt_rate.is_some()
        || body.star_rating.is_some()
        || body.modded_stats.is_some()
        || body.ar.is_some()
        || body.cs.is_some()
        || body.od.is_some()
        || body.hp.is_some()
        || body.bpm.is_some();
    if !any_update {
        return Err(AppError::bad_request("没有提供要更新的字段"));
    }

    // selectedMods 编码：提供 selectedMods 时按其编码；仅提供自定义字段时取基础 mod 重新编码
    let new_mods: Option<String> = if body.selected_mods.is_some()
        || body.custom_dt_rate.is_some()
        || body.custom_mod_name.is_some()
    {
        let base = body.selected_mods.clone();
        if let Some(mods) = &base {
            Some(MapSelection::encode_mods(
                mods,
                body.custom_dt_rate,
                body.custom_mod_name.as_deref(),
            ))
        } else {
            // 取当前值的基础 mod
            let current = run_db(&state.pool, move |conn| {
                map_selections::table
                    .find(id)
                    .select(map_selections::selected_mods)
                    .first::<Option<String>>(conn)
                    .optional()
                    .map_err(AppError::from)
            })
            .await?;
            let base_mod = current
                .flatten()
                .map(|s| MapSelection::base_mod(&s))
                .unwrap_or_else(|| "NM".into());
            Some(MapSelection::encode_mods(
                &base_mod,
                body.custom_dt_rate,
                body.custom_mod_name.as_deref(),
            ))
        }
    } else {
        None
    };

    // moddedStats 覆盖基础属性（原版二选一）
    let (ar, cs, od, hp, bpm) = if let Some(m) = &body.modded_stats {
        (
            m.get("ar").and_then(|v| v.as_f64()).or(body.ar),
            m.get("cs").and_then(|v| v.as_f64()).or(body.cs),
            m.get("od").and_then(|v| v.as_f64()).or(body.od),
            m.get("hp").and_then(|v| v.as_f64()).or(body.hp),
            m.get("bpm").and_then(|v| v.as_f64()).or(body.bpm),
        )
    } else {
        (body.ar, body.cs, body.od, body.hp, body.bpm)
    };
    let star_rating = body
        .modded_stats
        .as_ref()
        .and_then(|m| m.get("starRating").and_then(|v| v.as_f64()))
        .or(body.star_rating);
    let max_combo = body
        .modded_stats
        .as_ref()
        .and_then(|m| m.get("maxCombo").and_then(|v| v.as_i64()))
        .or(body.max_combo);

    let n = run_db(&state.pool, move |conn| {
        diesel::update(map_selections::table.find(id))
            .set((
                new_mods.map(|v| map_selections::selected_mods.eq(Some(v))),
                body.comment
                    .clone()
                    .map(|v| map_selections::comment.eq(Some(v))),
                body.approved.map(|v| map_selections::approved.eq(v)),
                body.padding.map(|v| map_selections::padding.eq(v)),
                body.title.clone().map(|v| map_selections::title.eq(v)),
                body.title_unicode
                    .clone()
                    .map(|v| map_selections::title_unicode.eq(Some(v))),
                body.artist.clone().map(|v| map_selections::artist.eq(v)),
                body.artist_unicode
                    .clone()
                    .map(|v| map_selections::artist_unicode.eq(Some(v))),
                body.version.clone().map(|v| map_selections::version.eq(v)),
                body.category
                    .clone()
                    .map(|v| map_selections::category.eq(Some(v))),
                body.total_length
                    .map(|v| map_selections::total_length.eq(v as i32)),
                max_combo.map(|v| map_selections::max_combo.eq(v as i32)),
                star_rating.map(|v| map_selections::star_rating.eq(v)),
                ar.map(|v| map_selections::ar.eq(v)),
                cs.map(|v| map_selections::cs.eq(v)),
                od.map(|v| map_selections::od.eq(v)),
                hp.map(|v| map_selections::hp.eq(v)),
                bpm.map(|v| map_selections::bpm.eq(v)),
                Some(map_selections::updated_at.eq(Utc::now().naive_utc())),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("更新选图失败或您没有权限更新此选图"));
    }
    Ok(ok_msg("选图更新成功", Value::Null))
}

// ============ DELETE /api/map-selections/{id} ============

async fn delete_selection(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    run_db(&state.pool, move |conn| {
        // 删除选图及其评论
        diesel::delete(map_comments::table.filter(map_comments::map_selection_id.eq(id)))
            .execute(conn)
            .map_err(AppError::from)?;
        diesel::delete(map_selections::table.find(id))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok_msg("选图删除成功", Value::Null))
}

// ============ POST /api/map-selections/bulk-approve ============

#[derive(serde::Deserialize)]
struct BulkApproveBody {
    #[serde(rename = "selectionIds")]
    selection_ids: Vec<i64>,
    approved: bool,
}

async fn bulk_approve(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Json(body): Json<BulkApproveBody>,
) -> ApiResult<Json<Value>> {
    if body.selection_ids.is_empty() {
        return Err(AppError::bad_request("缺少必要参数：selectionIds"));
    }
    let ids: Vec<i32> = body
        .selection_ids
        .iter()
        .filter_map(|v| i32::try_from(*v).ok())
        .collect();
    let n = run_db(&state.pool, move |conn| {
        diesel::update(map_selections::table.filter(map_selections::id.eq_any(ids)))
            .set((
                map_selections::approved.eq(body.approved),
                map_selections::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok_msg(
        &format!(
            "批量过审完成，成功: {n}，失败: {}",
            body.selection_ids.len() - n as usize
        ),
        json!({ "total": body.selection_ids.len(), "success": n, "failed": body.selection_ids.len() - n as usize }),
    ))
}

// ============ 评论（合并原 map-ratings 系） ============

#[derive(serde::Deserialize)]
struct CommentsQuery {
    #[serde(rename = "mapSelectionIds", alias = "ids", default)]
    ids: Option<String>,
    #[serde(rename = "mapSelectionId", default)]
    map_selection_id: Option<i64>,
}

async fn get_comments(
    State(state): State<AppState>,
    Query(q): Query<CommentsQuery>,
) -> ApiResult<Json<Value>> {
    if let Some(ids_param) = &q.ids {
        let ids: Vec<i32> = ids_param
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .filter(|v| *v > 0)
            .filter_map(|v| i32::try_from(v).ok())
            .collect();
        if ids.is_empty() {
            return Err(AppError::bad_request("无效的mapSelectionIds参数"));
        }
        let rows = run_db(&state.pool, move |conn| {
            map_comments::table
                .filter(map_comments::map_selection_id.eq_any(ids))
                .order(map_comments::created_at.desc())
                .load::<MapComment>(conn)
                .map_err(AppError::from)
        })
        .await?;
        let mut grouped: serde_json::Map<String, Value> = serde_json::Map::new();
        for r in &rows {
            grouped
                .entry(r.map_selection_id.to_string())
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .unwrap()
                .push(r.to_json());
        }
        Ok(ok(Value::Object(grouped)))
    } else if let Some(mid) = q.map_selection_id {
        let mid = i32::try_from(mid).map_err(|_| AppError::bad_request("无效的mapSelectionId"))?;
        let rows = run_db(&state.pool, move |conn| {
            map_comments::table
                .filter(map_comments::map_selection_id.eq(mid))
                .order(map_comments::created_at.desc())
                .load::<MapComment>(conn)
                .map_err(AppError::from)
        })
        .await?;
        Ok(ok(json!(
            rows.iter().map(|r| r.to_json()).collect::<Vec<_>>()
        )))
    } else {
        Err(AppError::bad_request("缺少必要参数：mapSelectionId"))
    }
}

#[derive(serde::Deserialize)]
struct AddCommentBody {
    #[serde(default)]
    comment: String,
}

async fn add_comment(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Path(map_selection_id): Path<i32>,
    Json(body): Json<AddCommentBody>,
) -> ApiResult<Json<Value>> {
    if body.comment.trim().is_empty() {
        return Err(AppError::bad_request("评论不能为空"));
    }
    run_db(&state.pool, move |conn| {
        let now = Utc::now().naive_utc();
        diesel::insert_into(map_comments::table)
            .values((
                map_comments::map_selection_id.eq(map_selection_id),
                map_comments::user_id.eq(&user.osu_id),
                map_comments::username.eq(&user.username),
                map_comments::avatar_url.eq(user.avatar_url.clone()),
                map_comments::comment.eq(body.comment.trim().to_string()),
                map_comments::created_at.eq(now),
                map_comments::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok_msg("评论添加成功", Value::Null))
}

async fn delete_comment(
    State(state): State<AppState>,
    user: crate::auth::SessionUser,
    Path(id): Path<i32>,
) -> ApiResult<Json<Value>> {
    let permissions = crate::routes::users_routes::permissions_json(&state, &user.osu_id).await?;
    let is_admin = permissions["isadmin"].as_bool().unwrap_or(false);
    let osu_id = user.osu_id.clone();
    let n = if is_admin {
        run_db(&state.pool, move |conn| {
            diesel::delete(map_comments::table.filter(map_comments::id.eq(id)))
                .execute(conn)
                .map_err(AppError::from)
        })
        .await?
    } else {
        run_db(&state.pool, move |conn| {
            diesel::delete(
                map_comments::table
                    .filter(map_comments::id.eq(id))
                    .filter(map_comments::user_id.eq(&osu_id)),
            )
            .execute(conn)
            .map_err(AppError::from)
        })
        .await?
    };
    if n == 0 {
        return Err(AppError::not_found("删除评论失败"));
    }
    Ok(ok_msg("评论删除成功", Value::Null))
}

// ============ osu! API beatmap 信息获取（供 POST 与 parse-url 使用） ============

/// 获取单个 beatmap 信息（字段映射与原版 getBeatmapInfo 一致）
pub async fn fetch_beatmap_info(state: &AppState, beatmap_id: i64) -> Result<Value, AppError> {
    let raw = state
        .osu
        .get_json(&format!("/beatmaps/{beatmap_id}"), &[])
        .await?;
    let title_unicode = raw["beatmapset"]["title_unicode"]
        .as_str()
        .map(|s| json!(s))
        .unwrap_or_else(|| raw["beatmapset"]["title"].clone());
    let artist_unicode = raw["beatmapset"]["artist_unicode"]
        .as_str()
        .map(|s| json!(s))
        .unwrap_or_else(|| raw["beatmapset"]["artist"].clone());
    let url = raw["url"]
        .as_str()
        .map(|s| json!(s))
        .unwrap_or_else(|| json!(format!("https://osu.ppy.sh/beatmaps/{beatmap_id}")));
    let cover_url = raw["beatmapset"]["covers"]["cover"]
        .as_str()
        .map(|s| json!(s))
        .unwrap_or_else(|| raw["beatmapset"]["covers"]["card"].clone());
    Ok(json!({
        "id": raw["id"],
        "beatmapsetId": raw["beatmapset_id"],
        "title": raw["beatmapset"]["title"],
        "title_unicode": title_unicode,
        "artist": raw["beatmapset"]["artist"],
        "artist_unicode": artist_unicode,
        "version": raw["version"],
        "creator": raw["beatmapset"]["creator"],
        "star_rating": raw["difficulty_rating"],
        "bpm": raw["bpm"],
        "total_length": raw["total_length"],
        "maxCombo": raw["max_combo"],
        "ar": raw["ar"],
        "cs": raw["cs"],
        "od": raw["accuracy"],
        "hp": raw["drain"],
        "url": url,
        "cover_url": cover_url,
    }))
}

/// 获取 beatmapset 全部难度（字段映射与原版 getBeatmapsetInfo 一致）
pub async fn fetch_beatmapset_info(
    state: &AppState,
    beatmapset_id: i64,
) -> Result<Vec<Value>, AppError> {
    let raw = state
        .osu
        .get_json(&format!("/beatmapsets/{beatmapset_id}"), &[])
        .await?;
    let empty = Vec::new();
    let beatmaps = raw["beatmaps"].as_array().unwrap_or(&empty);
    let title_unicode = raw["title_unicode"]
        .as_str()
        .map(|s| json!(s))
        .unwrap_or_else(|| raw["title"].clone());
    let artist_unicode = raw["artist_unicode"]
        .as_str()
        .map(|s| json!(s))
        .unwrap_or_else(|| raw["artist"].clone());
    let cover_url = raw["covers"]["cover"]
        .as_str()
        .map(|s| json!(s))
        .unwrap_or_else(|| raw["covers"]["card"].clone());
    let maps = beatmaps
        .iter()
        .map(|b| {
            let url = b["url"].as_str().map(|s| json!(s)).unwrap_or_else(|| {
                json!(format!(
                    "https://osu.ppy.sh/beatmaps/{}",
                    b["id"].as_i64().unwrap_or(0)
                ))
            });
            json!({
                "id": b["id"],
                "beatmapsetId": b["beatmapset_id"],
                "title": raw["title"],
                "title_unicode": title_unicode,
                "artist": raw["artist"],
                "artist_unicode": artist_unicode,
                "version": b["version"],
                "creator": raw["creator"],
                "star_rating": b["difficulty_rating"],
                "bpm": b["bpm"],
                "total_length": b["total_length"],
                "maxCombo": b["max_combo"],
                "ar": b["ar"],
                "cs": b["cs"],
                "od": b["accuracy"],
                "hp": b["drain"],
                "url": url,
                "cover_url": cover_url,
            })
        })
        .collect();
    Ok(maps)
}
