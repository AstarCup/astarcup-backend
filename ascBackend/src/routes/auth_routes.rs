// 统一认证路由：
//   GET  /api/auth/url           获取 osu! 授权跳转地址
//   GET  /api/auth/callback      OAuth 回调（签发 JWT cookie 并重定向首页）
//   POST /api/auth/logout        注销
//   GET  /api/auth/me            当前用户信息 + 权限（未登录返回 401）
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::auth::{
    SessionUser, authorize_url, clear_session_cookie, exchange_code, fetch_osu_me, session_cookie,
    upsert_osu_user,
};
use crate::db::run_db;
use crate::error::{ApiResult, AppError, ok};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/url", get(auth_url))
        .route("/api/auth/callback", get(callback))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
}

/// GET /api/auth/url —— 返回 osu! 授权地址
async fn auth_url(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(ok(json!({ "authUrl": authorize_url(&state.config) })))
}

/// GET /api/auth/callback?code=... —— OAuth 回调
async fn callback(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let frontend = state.config.frontend_url.trim_end_matches('/').to_string();
    let err_redirect = |code: &str| -> Response {
        Redirect::temporary(&format!("{frontend}/register?error={code}")).into_response()
    };

    if let Some(e) = params.get("error") {
        tracing::warn!("OAuth 回调返回错误: {e}");
        return err_redirect("auth_failed");
    }
    let Some(code) = params.get("code") else {
        return err_redirect("no_code");
    };

    let result = async {
        let token = exchange_code(&state.config, &state.http, code).await?;
        let me = fetch_osu_me(&state.http, &token.access_token).await?;
        let user = upsert_osu_user(&state.pool, &me).await?;
        let jwt = crate::auth::create_token(
            &state.config,
            &user.osu_id,
            &user.username,
            user.avatar_url.as_deref(),
        )?;
        Ok::<String, AppError>(jwt)
    }
    .await;

    match result {
        Ok(jwt) => {
            let mut resp = Redirect::temporary(&format!("{frontend}/")).into_response();
            if let Ok(v) = header::HeaderValue::from_str(&session_cookie(&state.config, &jwt)) {
                resp.headers_mut().append(header::SET_COOKIE, v);
            }
            resp
        }
        Err(e) => {
            tracing::error!("OAuth 回调失败: {}", e.message);
            err_redirect("token_failed")
        }
    }
}

/// POST /api/auth/logout
async fn logout(State(state): State<AppState>) -> Response {
    let mut resp = (
        StatusCode::OK,
        crate::error::ok_msg("退出登录成功", Value::Null),
    )
        .into_response();
    if let Ok(v) = header::HeaderValue::from_str(&clear_session_cookie(&state.config)) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

/// GET /api/auth/me —— 当前用户信息与权限（isplayer/isadmin）；未登录返回 401
async fn me(State(state): State<AppState>, user: SessionUser) -> ApiResult<Json<Value>> {
    let osu_id = user.osu_id.clone();
    let db_user = run_db(&state.pool, move |conn| {
        use crate::schema::users;
        use diesel::prelude::*;
        users::table
            .filter(users::osu_id.eq(&osu_id))
            .first::<crate::models::User>(conn)
            .optional()
            .map_err(AppError::from)
    })
    .await?;

    let Some(db_user) = db_user else {
        return Err(AppError::unauthorized("用户不存在"));
    };
    let is_admin = db_user.user_group == crate::enums::UserGroup::Admin;
    let is_registered = matches!(
        db_user.registration_status,
        crate::enums::RegistrationStatus::Registered | crate::enums::RegistrationStatus::Approved
    );
    Ok(ok(json!({
        "user": db_user.to_json(),
        "permissions": {
            "isplayer": !is_admin && is_registered,
            "isadmin": is_admin,
        },
    })))
}
