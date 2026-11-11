// 裁判控制台路由（配合前端 staff-dashboard「裁判控制台」Tab）
//
//   GET  /api/ref/bind/status          当前用户 osu! 裁判授权绑定状态
//   GET  /api/ref/bind/url             获取带裁判 scope 的 osu! 授权地址（state=ref:<nonce>）
//   POST /api/ref/bind/clear           解除绑定（删 token 并断开连接）
//   POST /api/ref/connect              建立/保持到 referee hub 的 SignalR 连接
//   GET  /api/ref/status               连接状态
//   POST /api/ref/invoke               调用 hub 命令（白名单，参数照 IRefereeHubServer）
//   GET  /api/ref/events               SSE：hub 事件 + 连接状态
//   POST /api/ref/chat/send            发聊天（用户自己 token）
//   GET  /api/ref/chat/messages        拉聊天记录
//   GET  /api/ref/beatmap/{id}         beatmap 信息代理
//
// 访问门槛：管理员或 staff_room_assignments 中的 referee 角色。
use std::convert::Infallible;

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use diesel::prelude::*;
use futures_util::stream::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::auth::{self, SessionUser};
use crate::db::run_db;
use crate::enums::{StaffRole, UserGroup};
use crate::error::{ApiResult, AppError, ok};
use crate::referee::REF_OAUTH_SCOPES;
use crate::state::AppState;

/// OAuth state 前缀：区分登录授权与裁判绑定授权（复用同一个回调地址）
pub const REF_STATE_PREFIX: &str = "ref:";
const REF_OAUTH_COOKIE: &str = "astra_ref_oauth";

/// 允许的 hub 命令（与 IRefereeHubServer 一致，见 apl-ref/src/referee/commands.js）
const ALLOWED_COMMANDS: &[&str] = &[
    "Ping",
    "MakeRoom",
    "JoinRoom",
    "CloseRoom",
    "InvitePlayer",
    "KickPlayer",
    "BanUser",
    "AddReferee",
    "RemoveReferee",
    "ListRooms",
    "ChangeRoomSettings",
    "EditCurrentPlaylistItem",
    "AddPlaylistItem",
    "EditPlaylistItem",
    "RemovePlaylistItem",
    "Roll",
    "MoveUser",
    "SetLockState",
    "StartMatch",
    "StopMatchCountdown",
    "AbortMatch",
];

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ref/bind/status", get(bind_status))
        .route("/api/ref/bind/url", get(bind_url))
        .route("/api/ref/bind/clear", post(bind_clear))
        .route("/api/ref/connect", post(connect))
        .route("/api/ref/status", get(status))
        .route("/api/ref/invoke", post(invoke))
        .route("/api/ref/events", get(events))
        .route("/api/ref/chat/send", post(chat_send))
        .route("/api/ref/chat/messages", get(chat_messages))
        .route("/api/ref/user/{user_id}", get(user))
        .route("/api/ref/beatmap/{beatmap_id}", get(beatmap))
}

/// 管理员或 referee 角色才能使用裁判控制台
async fn require_referee(state: &AppState, osu_id: &str) -> Result<(), AppError> {
    let uid = osu_id.to_string();
    run_db(&state.pool, move |conn| {
        use crate::schema::{staff_room_assignments, users};
        let is_admin = users::table
            .filter(users::osu_id.eq(&uid))
            .select(users::user_group)
            .first::<UserGroup>(conn)
            .optional()?
            == Some(UserGroup::Admin);
        if is_admin {
            return Ok(());
        }
        let is_ref = staff_room_assignments::table
            .filter(staff_room_assignments::staff_osu_id.eq(&uid))
            .filter(staff_room_assignments::role.eq(StaffRole::Referee))
            .count()
            .get_result::<i64>(conn)?
            > 0;
        if is_ref {
            Ok(())
        } else {
            Err(AppError::forbidden("需要裁判权限"))
        }
    })
    .await
}

// ============ 绑定 ============

async fn bind_status(State(state): State<AppState>, user: SessionUser) -> ApiResult<Json<Value>> {
    require_referee(&state, &user.osu_id).await?;
    let data = state.referee.bind_status(&user.osu_id).await?;
    Ok(ok(data))
}

/// GET /api/ref/bind/url —— 生成裁判 scope 的授权地址并种下 CSRF nonce cookie
async fn bind_url(State(state): State<AppState>, user: SessionUser) -> Response {
    if let Err(e) = require_referee(&state, &user.osu_id).await {
        return e.into_response();
    }
    let nonce = format!("{:032x}", rand::random::<u128>());
    let oauth_state = format!("{REF_STATE_PREFIX}{nonce}");
    let auth_url = auth::authorize_url_scoped(&state.config, Some(&oauth_state), REF_OAUTH_SCOPES);

    let mut resp = ok(json!({ "authUrl": auth_url })).into_response();
    let max_age = 600;
    let mut cookie =
        format!("{REF_OAUTH_COOKIE}={nonce}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}");
    if state.config.cookie_secure {
        cookie.push_str("; Secure");
    }
    if !state.config.cookie_domain.is_empty() {
        cookie.push_str(&format!("; Domain={}", state.config.cookie_domain));
    }
    if let Ok(v) = header::HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

/// POST /api/ref/bind/clear
async fn bind_clear(State(state): State<AppState>, user: SessionUser) -> ApiResult<Json<Value>> {
    require_referee(&state, &user.osu_id).await?;
    state.referee.clear_binding(&user.osu_id).await?;
    Ok(ok(json!({ "bound": false })))
}

/// 裁判授权回调（由 /api/auth/callback 在 state 以 ref: 开头时转进来）。
/// 校验 CSRF cookie → 换 token → 存 referee_tokens → 302 回 staff-dashboard。
pub async fn bind_callback(
    state: AppState,
    params: &std::collections::HashMap<String, String>,
    headers: &HeaderMap,
) -> Response {
    let frontend = state.config.frontend_url.trim_end_matches('/').to_string();
    let err_redirect = |code: &str| -> Response {
        Redirect::temporary(&format!("{frontend}/staff-dashboard?ref_error={code}")).into_response()
    };

    let Some(oauth_state) = params.get("state") else {
        return err_redirect("no_state");
    };
    let Some(nonce) = oauth_state.strip_prefix(REF_STATE_PREFIX) else {
        return err_redirect("bad_state");
    };
    // CSRF：state 中的 nonce 必须与回调携带来的 cookie 一致
    let cookie_nonce = auth::read_cookie(headers, REF_OAUTH_COOKIE);
    if cookie_nonce.as_deref() != Some(nonce) {
        return err_redirect("state_mismatch");
    }
    if let Some(e) = params.get("error") {
        tracing::warn!("裁判授权回调返回错误: {e}");
        return err_redirect("auth_failed");
    }
    let Some(code) = params.get("code") else {
        return err_redirect("no_code");
    };

    let result: Result<(), AppError> = async {
        let token = auth::exchange_code(&state.config, &state.http, code).await?;
        let me = auth::fetch_osu_me(&state.http, &token.access_token).await?;
        let osu_id = me["id"]
            .as_i64()
            .map(|v| v.to_string())
            .ok_or_else(|| AppError::internal("osu! 用户信息缺少 id"))?;
        let username = me["username"].as_str().unwrap_or("").to_string();
        // 绑定账号必须与当前登录账号一致（本站登录同样是 osu! 账号）
        if let Some(claims) = auth::session_user_from_headers(&state.config, headers) {
            if claims.osu_id != osu_id {
                return Err(AppError::forbidden(
                    "请使用与登录本站相同的 osu! 账号进行裁判授权",
                ));
            }
        }
        state.referee.save_binding(&osu_id, &username, &token).await
    }
    .await;

    match result {
        Ok(()) => {
            Redirect::temporary(&format!("{frontend}/staff-dashboard?ref_bound=1")).into_response()
        }
        Err(e) => {
            tracing::error!("裁判授权绑定失败: {}", e.message);
            let code = if e.status == StatusCode::BAD_GATEWAY {
                "osu_unreachable"
            } else if e.status == StatusCode::FORBIDDEN {
                "account_mismatch"
            } else {
                "token_failed"
            };
            err_redirect(code)
        }
    }
}

// ============ 连接与命令 ============

async fn connect(State(state): State<AppState>, user: SessionUser) -> ApiResult<Json<Value>> {
    require_referee(&state, &user.osu_id).await?;
    let st = state.referee.connect(&user.osu_id).await?;
    Ok(ok(json!(st)))
}

async fn status(State(state): State<AppState>, user: SessionUser) -> ApiResult<Json<Value>> {
    require_referee(&state, &user.osu_id).await?;
    let bound = state
        .referee
        .bind_status(&user.osu_id)
        .await?
        .get("bound")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let st = state.referee.status(&user.osu_id).await;
    Ok(ok(json!({
        "bound": bound,
        "state": st.state,
        "lastError": st.last_error,
    })))
}

#[derive(Deserialize)]
struct InvokeBody {
    command: String,
    #[serde(default)]
    args: Vec<Value>,
}

async fn invoke(
    State(state): State<AppState>,
    user: SessionUser,
    Json(body): Json<InvokeBody>,
) -> ApiResult<Json<Value>> {
    require_referee(&state, &user.osu_id).await?;
    if !ALLOWED_COMMANDS.contains(&body.command.as_str()) {
        return Err(AppError::bad_request(format!(
            "不支持的命令: {}",
            body.command
        )));
    }
    let result = state
        .referee
        .invoke(&user.osu_id, &body.command, body.args)
        .await?;
    Ok(ok(json!({ "result": result })))
}

/// GET /api/ref/events —— SSE：{"event","data"} 转发 hub 事件，连接状态单独 event
async fn events(
    State(state): State<AppState>,
    user: SessionUser,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    require_referee(&state, &user.osu_id).await?;
    let session = state.referee.ensure_session(&user.osu_id).await;
    let rx = session.events.subscribe();
    let state_rx = session.state_tx.subscribe();

    // 初始状态帧，方便前端立即渲染
    let initial_state = {
        let st = state_rx.borrow().clone();
        Event::default()
            .event("state")
            .data(serde_json::to_string(&st).unwrap_or_default())
    };

    let stream =
        futures_util::stream::unfold((rx, state_rx), |(mut rx, mut state_rx)| async move {
            loop {
                tokio::select! {
                    ev = rx.recv() => {
                        match ev {
                            Ok((name, data)) => {
                                let payload = json!({"event": name, "data": data});
                                return Some((
                                    Ok(Event::default().event("hub").data(payload.to_string())),
                                    (rx, state_rx),
                                ));
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => return None,
                        }
                    }
                    changed = state_rx.changed() => {
                        if changed.is_err() {
                            return None;
                        }
                        let st = state_rx.borrow_and_update().clone();
                        return Some((
                            Ok(Event::default().event("state").data(
                                serde_json::to_string(&st).unwrap_or_default(),
                            )),
                            (rx, state_rx),
                        ));
                    }
                }
            }
        });

    let stream = futures_util::stream::once(async move { Ok(initial_state) }).chain(stream);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

// ============ 聊天与 beatmap 代理 ============

#[derive(Deserialize)]
struct ChatSendBody {
    #[serde(rename = "channelId")]
    channel_id: i64,
    message: String,
}

async fn chat_send(
    State(state): State<AppState>,
    user: SessionUser,
    Json(body): Json<ChatSendBody>,
) -> ApiResult<Json<Value>> {
    require_referee(&state, &user.osu_id).await?;
    let msg = state
        .referee
        .chat_send(&user.osu_id, body.channel_id, &body.message)
        .await?;
    Ok(ok(json!({ "message": msg })))
}

#[derive(Deserialize)]
struct ChatMessagesQuery {
    #[serde(rename = "channelId")]
    channel_id: i64,
    after: Option<i64>,
}

async fn chat_messages(
    State(state): State<AppState>,
    user: SessionUser,
    Query(q): Query<ChatMessagesQuery>,
) -> ApiResult<Json<Value>> {
    require_referee(&state, &user.osu_id).await?;
    let messages = state
        .referee
        .chat_messages(&user.osu_id, q.channel_id, q.after)
        .await?;
    Ok(ok(json!({ "messages": messages })))
}

/// 用户信息代理（/api/v2/users/{id 或用户名}/osu）
async fn user(
    State(state): State<AppState>,
    user: SessionUser,
    Path(lookup): Path<String>,
) -> ApiResult<Json<Value>> {
    require_referee(&state, &user.osu_id).await?;
    let data = state.referee.user(&user.osu_id, &lookup).await?;
    Ok(ok(data))
}

async fn beatmap(
    State(state): State<AppState>,
    user: SessionUser,
    Path(beatmap_id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_referee(&state, &user.osu_id).await?;
    let id: i64 = beatmap_id
        .parse()
        .map_err(|_| AppError::bad_request("beatmap id 无效"))?;
    let data = state.referee.beatmap(id).await?;
    Ok(ok(data))
}
