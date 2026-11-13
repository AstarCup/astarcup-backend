// 计划任务管理（管理员）
//   GET   /api/admin/scheduled-tasks            任务列表（含上次执行情况）
//   PATCH /api/admin/scheduled-tasks/{name}     修改配置（enabled / intervalSecs / startTime / runAmount / params）
//   POST  /api/admin/scheduled-tasks/{name}/run 立即执行一次
use axum::extract::{Path, State};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::AdminUser;
use crate::db::run_db;
use crate::error::{ApiResult, AppError, ok};
use crate::models::ScheduledTask;
use crate::schema::scheduled_tasks;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/scheduled-tasks", get(list_tasks))
        .route("/api/admin/scheduled-tasks/{name}", patch(update_task))
        .route("/api/admin/scheduled-tasks/{name}/run", post(run_task_now))
}

// ============ GET /api/admin/scheduled-tasks ============

async fn list_tasks(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<Value>> {
    let tasks = run_db(&state.pool, |conn| {
        scheduled_tasks::table
            .order(scheduled_tasks::id.asc())
            .load::<ScheduledTask>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok(json!({
        "tasks": tasks.iter().map(|t| t.to_json()).collect::<Vec<_>>(),
    })))
}

// ============ PATCH /api/admin/scheduled-tasks/{name} ============

#[derive(Deserialize)]
struct UpdateTaskBody {
    enabled: Option<bool>,
    #[serde(rename = "intervalSecs")]
    interval_secs: Option<i32>,
    #[serde(rename = "startTime")]
    start_time: Option<String>,
    #[serde(rename = "runAmount")]
    run_amount: Option<i32>,
    params: Option<Value>,
}

async fn update_task(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(name): Path<String>,
    Json(body): Json<UpdateTaskBody>,
) -> ApiResult<Json<Value>> {
    if let Some(v) = body.interval_secs {
        if v < 1 {
            return Err(AppError::bad_request("intervalSecs 必须 ≥ 1"));
        }
    }
    if let Some(v) = body.run_amount {
        if v < 1 {
            return Err(AppError::bad_request("runAmount 必须 ≥ 1"));
        }
    }
    if let Some(t) = &body.start_time {
        if chrono::NaiveTime::parse_from_str(t, "%H:%M").is_err() {
            return Err(AppError::bad_request("startTime 需为 HH:MM 格式"));
        }
    }

    let updated = run_db(&state.pool, move |conn| {
        let target = scheduled_tasks::table
            .filter(scheduled_tasks::name.eq(&name))
            .first::<ScheduledTask>(conn)
            .optional()
            .map_err(AppError::from)?;
        let Some(target) = target else {
            return Err(AppError::not_found("计划任务不存在"));
        };
        // 局部更新：未提供的字段保持原值
        diesel::update(scheduled_tasks::table.find(target.id))
            .set((
                scheduled_tasks::enabled.eq(body.enabled.unwrap_or(target.enabled)),
                scheduled_tasks::interval_secs
                    .eq(body.interval_secs.unwrap_or(target.interval_secs)),
                scheduled_tasks::start_time
                    .eq(body.start_time.unwrap_or(target.start_time)),
                scheduled_tasks::run_amount.eq(body.run_amount.unwrap_or(target.run_amount)),
                scheduled_tasks::params.eq(body.params.unwrap_or(target.params)),
                scheduled_tasks::updated_at.eq(Utc::now().naive_utc()),
            ))
            .get_result::<ScheduledTask>(conn)
            .map_err(AppError::from)
    })
    .await?;
    Ok(ok(json!({ "task": updated.to_json() })))
}

// ============ POST /api/admin/scheduled-tasks/{name}/run ============

async fn run_task_now(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(name): Path<String>,
) -> ApiResult<Json<Value>> {
    let task = run_db(&state.pool, move |conn| {
        scheduled_tasks::table
            .filter(scheduled_tasks::name.eq(name))
            .first::<ScheduledTask>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?
    .ok_or_else(|| AppError::not_found("计划任务不存在"))?;
    let note = crate::scheduler::run_now(&state, &task).await?;
    Ok(ok(json!({ "note": note })))
}
