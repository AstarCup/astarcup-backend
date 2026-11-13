//! 计划任务调度器：按 scheduled_tasks 表的配置周期性执行内置任务。
//!
//! - 每 30s 扫描一次启用中的任务，按 interval_secs / start_time 判断是否到期；
//! - 到期则执行对应处理器，并把结果写回 last_run_at / last_run_note（管理后台可见）；
//! - 单实例语义：多副本部署会重复执行（见 README「部署注意」）。

use chrono::{Duration as ChronoDuration, Local, NaiveTime, Utc};
use diesel::prelude::*;

use crate::db::{run_db, DbPool};
use crate::error::AppError;
use crate::models::ScheduledTask;
use crate::schema::{scheduled_tasks, users};
use crate::state::AppState;

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            ticker.tick().await;
            if let Err(msg) = tick(&state).await {
                tracing::warn!("计划任务调度失败: {msg}");
            }
        }
    });
}

/// 立即执行一次指定任务（管理后台「立即执行」按钮）；返回运行摘要
pub async fn run_now(state: &AppState, task: &ScheduledTask) -> Result<String, AppError> {
    let note = run_task(state, task).await;
    record_run(&state.pool, &task.name, &note).await;
    Ok(note)
}

async fn tick(state: &AppState) -> Result<(), String> {
    let tasks = run_db(&state.pool, |conn| {
        scheduled_tasks::table
            .filter(scheduled_tasks::enabled.eq(true))
            .load::<ScheduledTask>(conn)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| e.message)?;

    for task in tasks {
        if !due(&task) {
            continue;
        }
        let note = run_task(state, &task).await;
        record_run(&state.pool, &task.name, &note).await;
    }
    Ok(())
}

/// 是否到期：到达每日起始锚点（本地时间 HH:MM）后，距上次执行满 interval_secs 即到期
fn due(task: &ScheduledTask) -> bool {
    if let Ok(t) = NaiveTime::parse_from_str(&task.start_time, "%H:%M") {
        if Local::now().time() < t {
            return false;
        }
    }
    match task.last_run_at {
        None => true,
        Some(last) => {
            (Utc::now().naive_utc() - last).num_seconds() >= i64::from(task.interval_secs)
        }
    }
}

async fn run_task(state: &AppState, task: &ScheduledTask) -> String {
    match task.name.as_str() {
        "cover_refresh" => cover_refresh(state, task).await,
        other => {
            tracing::warn!("未知计划任务类型: {other}");
            "未知任务类型，已跳过".to_string()
        }
    }
}

async fn record_run(pool: &DbPool, name: &str, note: &str) {
    let name = name.to_string();
    let note = note.to_string();
    let _ = run_db(pool, move |conn| {
        diesel::update(scheduled_tasks::table.filter(scheduled_tasks::name.eq(name)))
            .set((
                scheduled_tasks::last_run_at.eq(Utc::now().naive_utc()),
                scheduled_tasks::last_run_note.eq(note),
                scheduled_tasks::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)
            .map_err(AppError::from)?;
        Ok(())
    })
    .await;
}

/// 用户 cover 刷新：每次处理 run_amount 个「cover 最久未刷新」用户。
///
/// params.stale_hours（默认 24）：刷新未满该时长的用户跳过——配合每分钟一次的
/// 触发节奏，实现「每天 0 点起、每分钟 1 个用户」的均匀刷新。
async fn cover_refresh(state: &AppState, task: &ScheduledTask) -> String {
    let stale_hours = task
        .params
        .get("stale_hours")
        .and_then(|v| v.as_i64())
        .unwrap_or(24);
    let cutoff = Utc::now().naive_utc() - ChronoDuration::hours(stale_hours);
    let amount = i64::from(task.run_amount.max(1));

    let targets = run_db(&state.pool, move |conn| {
        users::table
            .select(users::osu_id)
            .filter(
                users::cover_updated_at
                    .is_null()
                    .or(users::cover_updated_at.lt(cutoff)),
            )
            .order(users::cover_updated_at.asc().nulls_first())
            .limit(amount)
            .load::<String>(conn)
            .map_err(AppError::from)
    })
    .await;

    let targets = match targets {
        Ok(t) => t,
        Err(e) => return format!("查询待刷新用户失败: {}", e.message),
    };
    if targets.is_empty() {
        return "无待刷新用户".to_string();
    }

    let mut ok_count = 0usize;
    let mut fail_count = 0usize;
    for (i, osu_id) in targets.iter().enumerate() {
        if i > 0 {
            // 节流：osu! API client-credentials 限速约 60 次/分
            tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        }
        match state.osu.get_json(&format!("/users/{osu_id}"), &[]).await {
            Ok(me) => match crate::auth::upsert_osu_user(&state.pool, &me).await {
                Ok(_) => ok_count += 1,
                Err(e) => {
                    fail_count += 1;
                    tracing::warn!("刷新用户 {osu_id} cover 落库失败: {}", e.message);
                }
            },
            Err(e) if e.status == axum::http::StatusCode::NOT_FOUND => {
                // osu! 账号不存在（已注销/受限）：推进刷新时间戳移出队列，stale_hours 后复查
                tracing::info!("用户 {osu_id} 在 osu! 不存在（可能已注销/受限），本轮跳过");
                touch_cover_updated_at(&state.pool, osu_id).await;
            }
            Err(e) => {
                fail_count += 1;
                tracing::warn!("拉取用户 {osu_id} cover 失败: {}", e.message);
            }
        }
    }
    if fail_count > 0 {
        format!("刷新成功 {ok_count} 个用户（失败 {fail_count}）")
    } else {
        format!("刷新成功 {ok_count} 个用户")
    }
}

/// 仅推进 cover 刷新时间戳（把用户移出本轮待刷队列）
async fn touch_cover_updated_at(pool: &DbPool, osu_id: &str) {
    let osu_id = osu_id.to_string();
    let _ = run_db(pool, move |conn| {
        diesel::update(users::table.filter(users::osu_id.eq(osu_id)))
            .set(users::cover_updated_at.eq(Utc::now().naive_utc()))
            .execute(conn)
            .map_err(AppError::from)?;
        Ok(())
    })
    .await;
}
