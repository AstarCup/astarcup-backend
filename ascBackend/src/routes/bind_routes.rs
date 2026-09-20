// g0v0 私服绑定路由（OAuth2 与 osu! API v2 一致）
//
//   GET  /api/bind/url        获取 g0v0 授权地址（需登录）
//   GET  /api/bind/callback   OAuth 回调：换取 token、拉取 /api/v2/me、写库绑定
//   POST /api/bind/unbind     解绑（需登录）
use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use diesel::prelude::*;
use serde_json::{Value, json};

use crate::auth::{SessionUser, url_encode};
use crate::db::run_db;
use crate::error::{ApiResult, AppError, ok, ok_msg};
use crate::schema::users;
use crate::state::AppState;

const STATE_COOKIE: &str = "g0v0_state";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/bind/url", get(bind_url))
        .route("/api/bind/callback", get(bind_callback))
        .route("/api/bind/unbind", axum::routing::post(unbind))
}

fn g0v0_ready(cfg: &crate::config::Config) -> Result<(), AppError> {
    if !cfg.g0v0_enabled {
        return Err(AppError::bad_request("g0v0 绑定功能未启用"));
    }
    if cfg.g0v0_client_id.is_empty() || cfg.g0v0_client_secret.is_empty() {
        return Err(AppError::internal(
            "g0v0 OAuth 尚未配置（G0V0_CLIENT_ID/G0V0_CLIENT_SECRET）",
        ));
    }
    Ok(())
}

/// 极简随机 state（防 CSRF）：时间戳 + 用户维度散列
fn gen_state(osu_id: &str) -> String {
    let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let h = osu_id
        .bytes()
        .fold(0x811c9dc5u32, |acc, b| (acc ^ b as u32).wrapping_mul(0x01000193));
    format!("{ts:x}-{h:x}")
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|raw| {
            raw.split(';')
                .map(|p| p.trim())
                .find_map(|p| p.strip_prefix(&format!("{name}=")))
                .map(|s| s.to_string())
        })
}

/// 截断过长的响应原文，避免日志刷屏
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…（共 {} 字符）", s.chars().count())
    }
}

// ============ GET /api/bind/url ============

async fn bind_url(State(state): State<AppState>, user: SessionUser) -> ApiResult<Response> {
    g0v0_ready(&state.config)?;
    let cfg = &state.config;
    let state_str = gen_state(&user.osu_id);
    let auth_url = format!(
        "{}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify&state={}",
        cfg.g0v0_oauth_base.trim_end_matches('/'),
        url_encode(&cfg.g0v0_client_id),
        url_encode(&cfg.g0v0_redirect_uri),
        url_encode(&state_str),
    );

    let mut resp = ok(json!({ "authUrl": auth_url })).into_response();
    if let Ok(v) = header::HeaderValue::from_str(&format!(
        "{STATE_COOKIE}={state_str}; Path=/api/bind; HttpOnly; SameSite=Lax; Max-Age=600"
    )) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    Ok(resp)
}

// ============ GET /api/bind/callback ============

async fn bind_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: SessionUser,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let frontend = state.config.frontend_url.trim_end_matches('/').to_string();
    let fail = |code: &str| -> Response {
        Redirect::temporary(&format!("{frontend}/player-info?bound=error&reason={code}"))
            .into_response()
    };

    if !state.config.g0v0_enabled {
        return fail("disabled");
    }
    let state_cookie = cookie_value(&headers, STATE_COOKIE);
    if state_cookie.as_deref() != params.get("state").map(|s| s.as_str()) {
        return fail("state_mismatch");
    }
    let Some(code) = params.get("code") else {
        return fail("no_code");
    };

    // 换取 token
    let cfg = &state.config;
    let resp = state
        .http
        .post(format!("{}/oauth/token", cfg.g0v0_oauth_base.trim_end_matches('/')))
        .form(&[
            ("client_id", cfg.g0v0_client_id.as_str()),
            ("client_secret", cfg.g0v0_client_secret.as_str()),
            ("code", code.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", cfg.g0v0_redirect_uri.as_str()),
        ])
        .send()
        .await;
    let Ok(resp) = resp else {
        return fail("network");
    };
    let status = resp.status();
    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return fail("token_parse"),
    };
    if !status.is_success() {
        tracing::warn!("g0v0 token 换取失败: {body}");
        return fail("token_failed");
    }
    let Some(access_token) = body["access_token"].as_str().map(|s| s.to_string()) else {
        return fail("token_missing");
    };
    let refresh_token = body["refresh_token"].as_str().map(|s| s.to_string());

    // 拉取 g0v0 用户信息（/api/v2/me/ —— 注意必须带尾斜杠，否则服务器会 307 到外部 IP 并丢失鉴权头）
    let me_resp = state
        .http
        .get(format!(
            "{}/api/v2/me/",
            cfg.g0v0_oauth_base.trim_end_matches('/')
        ))
        .bearer_auth(&access_token)
        .send()
        .await;
    let Ok(me_resp) = me_resp else {
        return fail("network");
    };
    let me_status = me_resp.status();
    let me_text = match me_resp.text().await {
        Ok(t) => t,
        Err(_) => return fail("me_parse"),
    };
    if !me_status.is_success() {
        tracing::warn!(
            "g0v0 /me HTTP {} 原文: {}",
            me_status,
            truncate(&me_text, 600)
        );
        return fail("me_failed");
    }
    let me: Value = match serde_json::from_str(me_text.trim_start_matches('\u{feff}')) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(
                "g0v0 /me 解析失败: {e} 原文: {}",
                truncate(&me_text, 600)
            );
            return fail("me_parse");
        }
    };
    let gu_id = me["id"]
        .as_i64()
        .map(|v| v.to_string())
        .or_else(|| me["id"].as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    let gu_username = me["username"].as_str().unwrap_or("").to_string();
    if gu_id.is_empty() {
        tracing::error!("g0v0 /me 缺少 id: {}", truncate(&me_text, 600));
        return fail("me_id_missing");
    }

    // 写库绑定
    let osu_id = user.osu_id.clone();
    let now = Utc::now().naive_utc();
    let upd = run_db(&state.pool, move |conn| {
        diesel::update(users::table.filter(users::osu_id.eq(&osu_id)))
            .set((
                users::gu_server_user_id.eq(Some(&gu_id)),
                users::gu_server_username.eq(Some(&gu_username)),
                users::gu_server_bind_at.eq(Some(now)),
                users::gu_refresh_token.eq(refresh_token),
                users::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await;

    if let Err(e) = upd {
        tracing::error!("g0v0 绑定写库失败: {}", e.message);
        return fail("db_error");
    }

    // 清除 state cookie 并跳转前端
    let mut resp =
        Redirect::temporary(&format!("{frontend}/player-info?bound=1")).into_response();
    if let Ok(v) = header::HeaderValue::from_str(&format!(
        "{STATE_COOKIE}=; Path=/api/bind; HttpOnly; SameSite=Lax; Max-Age=0"
    )) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

// ============ POST /api/bind/unbind ============

async fn unbind(State(state): State<AppState>, user: SessionUser) -> ApiResult<Json<Value>> {
    let osu_id = user.osu_id.clone();
    let now = Utc::now().naive_utc();
    let n = run_db(&state.pool, move |conn| {
        diesel::update(users::table.filter(users::osu_id.eq(&osu_id)))
            .set((
                users::gu_server_user_id.eq(None::<String>),
                users::gu_server_username.eq(None::<String>),
                users::gu_server_bind_at.eq(None::<chrono::NaiveDateTime>),
                users::gu_refresh_token.eq(None::<String>),
                users::updated_at.eq(now),
            ))
            .execute(conn)
            .map_err(AppError::from)
    })
    .await?;
    if n == 0 {
        return Err(AppError::not_found("用户不存在"));
    }
    Ok(ok_msg("已解除 g0v0 绑定", Value::Null))
}
