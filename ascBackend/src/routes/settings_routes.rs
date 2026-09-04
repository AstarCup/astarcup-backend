// 统一设置路由（合并原 tournament-settings、season-config、rank-config）
//
//   GET /api/settings   合并返回比赛设置 + 赛季配置 + 段位限制配置
//   PUT /api/settings   更新比赛设置键值（管理员）
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::db::run_db;
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::models::TournamentSetting;
use crate::schema::tournament_settings;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/settings", get(get_settings).put(update_settings))
}

fn default_settings() -> Vec<(&'static str, Value)> {
    vec![
        ("tournament_name", json!("")),
        ("max_pp_for_registration", json!(7000)),
        ("min_pp_for_registration", json!(0)),
        ("current_season", json!("s1")),
        ("current_season_stage", json!("registration")),
        ("registration_enabled", json!(true)),
        ("mappool_visible", json!(false)),
    ]
}

/// 将字符串值转换为合适的 JSON 类型（与原版解析逻辑一致）
fn parse_value(raw: &str) -> Value {
    if let Ok(v) = serde_json::from_str::<Value>(raw) {
        return v;
    }
    if let Ok(n) = raw.parse::<f64>() {
        if !raw.trim().is_empty() {
            return json!(n);
        }
    }
    match raw {
        "true" => json!(true),
        "false" => json!(false),
        _ => json!(raw),
    }
}

// ============ GET /api/settings ============

async fn get_settings(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let rows = run_db(&state.pool, |conn| {
        tournament_settings::table
            .load::<TournamentSetting>(conn)
            .map_err(AppError::from)
    })
    .await?;
    let mut settings = serde_json::Map::new();
    for (k, v) in default_settings() {
        settings.insert(k.to_string(), v);
    }
    for row in &rows {
        settings.insert(row.setting_key.clone(), parse_value(&row.setting_value));
    }
    // 附带可选赛季列表（UI 元数据）
    settings.insert(
        "availableSeasons".to_string(),
        json!([
            { "value": "s1", "label": "Season 1" },
            { "value": "otc1", "label": "OTC#1" },
        ]),
    );
    Ok(ok(Value::Object(settings)))
}

// ============ PUT /api/settings ============

async fn update_settings(
    State(state): State<AppState>,
    _admin: crate::auth::AdminUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    // 支持平铺对象或 { settings: {...} } 两种形式
    let obj = if let Some(s) = body.get("settings") {
        s.clone()
    } else {
        body.clone()
    };
    let Some(map) = obj.as_object() else {
        return Err(AppError::bad_request("无效的设置数据"));
    };
    if map
        .get("tournament_name")
        .and_then(|v| v.as_str())
        .map(|s| s.is_empty())
        .unwrap_or(false)
    {
        return Err(AppError::bad_request("比赛名称不能为空"));
    }
    let pairs: Vec<(String, String)> = map
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| match v {
            Value::String(s) => (k.clone(), s.clone()),
            other => (k.clone(), other.to_string()),
        })
        .collect();
    if pairs.is_empty() {
        return Err(AppError::bad_request("没有要更新的设置"));
    }
    run_db(&state.pool, move |conn| {
        conn.transaction(|txn| {
            for (key, value) in &pairs {
                // upsert：存在则更新，不存在则插入
                let existing: Option<i32> = tournament_settings::table
                    .filter(tournament_settings::setting_key.eq(key))
                    .select(tournament_settings::id)
                    .first(txn)
                    .optional()
                    .map_err(AppError::from)?;
                let now = chrono::Utc::now().naive_utc();
                if let Some(id) = existing {
                    diesel::update(tournament_settings::table.find(id))
                        .set((
                            tournament_settings::setting_value.eq(value),
                            tournament_settings::updated_at.eq(now),
                        ))
                        .execute(txn)
                        .map_err(AppError::from)?;
                } else {
                    diesel::insert_into(tournament_settings::table)
                        .values((
                            tournament_settings::setting_key.eq(key),
                            tournament_settings::setting_value.eq(value),
                            tournament_settings::created_at.eq(now),
                            tournament_settings::updated_at.eq(now),
                        ))
                        .execute(txn)
                        .map_err(AppError::from)?;
                }
            }
            Ok(())
        })
    })
    .await?;
    Ok(ok_msg("比赛设置更新成功", Value::Null))
}
