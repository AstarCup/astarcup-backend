// 裁判控制台服务：维护每位工作人员到 osu! referee hub 的 SignalR 连接
//
// 方案 B：每个 staff 用自己的 osu! 账号授权（referee_tokens 表存 access/refresh token），
// 后端代持令牌、自动续期，并把 hub 事件通过 SSE 转发给前端控制台。
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use diesel::prelude::*;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{RwLock, broadcast, watch};

use super::signalr::{self, SignalRConnection};
use crate::auth::OsuToken;
use crate::config::Config;
use crate::db::{DbPool, run_db};
use crate::error::AppError;
use crate::models::RefereeToken;
use crate::osu_client::OsuClient;
use crate::schema::referee_tokens;

/// osu! 授权 scope：裁判 hub 需要 multiplayer.write_manage，聊天面板需要 chat.read/write
pub const REF_OAUTH_SCOPES: &str = "public identify chat.read chat.write multiplayer.write_manage";

const OSU_API_BASE: &str = "https://osu.ppy.sh/api/v2";
const TOKEN_URL: &str = "https://osu.ppy.sh/oauth/token";
const HUB_BASE: &str = "https://spectator.ppy.sh";
const HUB_PATH: &str = "/referee";
const INVOKE_TIMEOUT: Duration = Duration::from_secs(15);
/// 令牌过期前 60 秒视为无效，避免请求中途过期
const TOKEN_MARGIN_SECS: i64 = 60;

/// 连接状态（SSE "state" 事件 / status 接口共用）
#[derive(Debug, Clone, Serialize)]
pub struct ConnState {
    pub state: String, // connected | connecting | disconnected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl ConnState {
    fn new(state: &str, last_error: Option<String>) -> Self {
        ConnState {
            state: state.to_string(),
            last_error,
        }
    }
}

/// 单个用户的连接会话。hub 事件为 (事件名, 参数)。
pub struct Session {
    pub events: broadcast::Sender<(String, Value)>,
    pub state_tx: watch::Sender<ConnState>,
    conn: RwLock<Option<Arc<SignalRConnection>>>,
    intent: AtomicBool,
}

impl Session {
    fn new() -> Self {
        let (events, _) = broadcast::channel(256);
        let (state_tx, _) = watch::channel(ConnState::new("disconnected", None));
        Session {
            events,
            state_tx,
            conn: RwLock::new(None),
            intent: AtomicBool::new(false),
        }
    }

    fn set_state(&self, state: &str, last_error: Option<String>) {
        let _ = self.state_tx.send(ConnState::new(state, last_error));
    }

    pub fn current_state(&self) -> ConnState {
        self.state_tx.borrow().clone()
    }
}

pub struct RefereeService {
    pool: DbPool,
    config: Arc<Config>,
    http: reqwest::Client,
    /// client_credentials 客户端（beatmap 查询等公开接口）
    osu: Arc<OsuClient>,
    sessions: tokio::sync::Mutex<HashMap<String, Arc<Session>>>,
}

impl RefereeService {
    pub fn new(
        pool: DbPool,
        config: Arc<Config>,
        http: reqwest::Client,
        osu: Arc<OsuClient>,
    ) -> Self {
        RefereeService {
            pool,
            config,
            http,
            osu,
            sessions: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    // ============ 会话管理 ============

    /// 获取或创建会话（SSE 订阅者即使未连接也需要会话对象）
    pub async fn ensure_session(self: &Arc<Self>, osu_id: &str) -> Arc<Session> {
        let mut map = self.sessions.lock().await;
        map.entry(osu_id.to_string())
            .or_insert_with(|| Arc::new(Session::new()))
            .clone()
    }

    pub async fn get_session(&self, osu_id: &str) -> Option<Arc<Session>> {
        self.sessions.lock().await.get(osu_id).cloned()
    }

    /// 建立（或保持）到 referee hub 的连接。幂等，可重复调用。
    pub async fn connect(self: &Arc<Self>, osu_id: &str) -> Result<ConnState, AppError> {
        let session = self.ensure_session(osu_id).await;
        if !session.intent.swap(true, Ordering::SeqCst) {
            let svc = self.clone();
            let uid = osu_id.to_string();
            tokio::spawn(async move { pump(svc, uid).await });
        }
        // 等待一小段时间让首连结果反映出来（不阻塞到连接成功）
        let mut rx = session.state_tx.subscribe();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            let st = rx.borrow().clone();
            if st.state == "connected" {
                return Ok(st);
            }
            if tokio::time::timeout_at(deadline, rx.changed())
                .await
                .is_err()
            {
                return Ok(session.current_state());
            }
        }
    }

    /// 断开并清除会话（解除绑定时调用）
    pub async fn disconnect(&self, osu_id: &str) {
        if let Some(session) = self.get_session(osu_id).await {
            session.intent.store(false, Ordering::SeqCst);
            if let Some(conn) = session.conn.write().await.take() {
                conn.close().await;
            }
            session.set_state("disconnected", None);
        }
        self.sessions.lock().await.remove(osu_id);
    }

    pub async fn status(&self, osu_id: &str) -> ConnState {
        self.get_session(osu_id)
            .await
            .map(|s| s.current_state())
            .unwrap_or_else(|| ConnState::new("disconnected", None))
    }

    // ============ 命令调用 ============

    pub async fn invoke(
        &self,
        osu_id: &str,
        command: &str,
        args: Vec<Value>,
    ) -> Result<Value, AppError> {
        let session = self
            .get_session(osu_id)
            .await
            .ok_or_else(|| AppError::bad_request("尚未连接裁判服务器"))?;
        let conn = session
            .conn
            .read()
            .await
            .clone()
            .ok_or_else(|| AppError::bad_request("尚未连接裁判服务器"))?;
        tokio::time::timeout(INVOKE_TIMEOUT, conn.invoke(command, args))
            .await
            .map_err(|_| AppError::internal("裁判服务器响应超时"))?
            .map_err(AppError::bad_request)
    }

    // ============ osu! OAuth token 管理 ============

    async fn load_token(&self, osu_id: &str) -> Result<Option<RefereeToken>, AppError> {
        let uid = osu_id.to_string();
        run_db(&self.pool, move |conn| {
            referee_tokens::table
                .filter(referee_tokens::osu_id.eq(&uid))
                .first::<RefereeToken>(conn)
                .optional()
                .map_err(AppError::from)
        })
        .await
    }

    pub async fn bind_status(&self, osu_id: &str) -> Result<Value, AppError> {
        Ok(match self.load_token(osu_id).await? {
            Some(t) => t.to_json(),
            None => json!({"bound": false}),
        })
    }

    /// 绑定（授权回调后保存 token）
    pub async fn save_binding(
        &self,
        osu_id: &str,
        username: &str,
        token: &OsuToken,
    ) -> Result<(), AppError> {
        let now = Utc::now().naive_utc();
        let expires_at = now + ChronoDuration::seconds(token.expires_in.unwrap_or(86400));
        let uid = osu_id.to_string();
        let uname = username.to_string();
        let access = token.access_token.clone();
        let refresh = token.refresh_token.clone();
        let scope = REF_OAUTH_SCOPES.to_string();
        run_db(&self.pool, move |conn| {
            diesel::delete(referee_tokens::table.filter(referee_tokens::osu_id.eq(&uid)))
                .execute(conn)?;
            diesel::insert_into(referee_tokens::table)
                .values((
                    referee_tokens::osu_id.eq(&uid),
                    referee_tokens::username.eq(&uname),
                    referee_tokens::access_token.eq(&access),
                    referee_tokens::refresh_token.eq(&refresh),
                    referee_tokens::expires_at.eq(expires_at),
                    referee_tokens::scope.eq(&scope),
                    referee_tokens::bound_at.eq(now),
                    referee_tokens::updated_at.eq(now),
                ))
                .execute(conn)?;
            Ok(())
        })
        .await
    }

    /// 解除绑定：删 token 并断开连接
    pub async fn clear_binding(&self, osu_id: &str) -> Result<(), AppError> {
        self.disconnect(osu_id).await;
        let uid = osu_id.to_string();
        run_db(&self.pool, move |conn| {
            diesel::delete(referee_tokens::table.filter(referee_tokens::osu_id.eq(&uid)))
                .execute(conn)?;
            Ok(())
        })
        .await
    }

    /// 取可用的 access token（过期自动用 refresh_token 续期）
    pub async fn ensure_token(&self, osu_id: &str) -> Result<String, AppError> {
        let row = self
            .load_token(osu_id)
            .await?
            .ok_or_else(|| AppError::unauthorized("尚未绑定 osu! 裁判授权"))?;
        let now = Utc::now().naive_utc();
        if row.expires_at > now + ChronoDuration::seconds(TOKEN_MARGIN_SECS) {
            return Ok(row.access_token);
        }
        self.refresh_token(&row).await
    }

    async fn refresh_token(&self, row: &RefereeToken) -> Result<String, AppError> {
        let Some(refresh) = row.refresh_token.as_deref() else {
            return Err(AppError::unauthorized(
                "osu! 授权已过期且无 refresh token，请重新绑定",
            ));
        };
        let resp = self
            .http
            .post(TOKEN_URL)
            .form(&[
                ("client_id", self.config.osu_client_id.as_str()),
                ("client_secret", self.config.osu_client_secret.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh),
            ])
            .send()
            .await
            .map_err(|e| AppError::bad_gateway(format!("刷新 osu! token 失败: {e}")))?;
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("解析 osu! token 响应失败: {e}")))?;
        if !status.is_success() {
            tracing::warn!("referee token 刷新失败: {body}");
            return Err(AppError::unauthorized(
                "osu! 授权已失效，请重新绑定裁判授权",
            ));
        }
        let token: OsuToken = serde_json::from_value(body)
            .map_err(|e| AppError::internal(format!("osu! token 响应格式错误: {e}")))?;
        self.save_binding(&row.osu_id, &row.username, &token)
            .await?;
        Ok(token.access_token)
    }

    // ============ osu! REST 代理 ============

    /// 发聊天（用用户自己的 token）
    pub async fn chat_send(
        &self,
        osu_id: &str,
        channel_id: i64,
        message: &str,
    ) -> Result<Value, AppError> {
        let token = self.ensure_token(osu_id).await?;
        let resp = self
            .http
            .post(format!(
                "{OSU_API_BASE}/chat/channels/{channel_id}/messages"
            ))
            .bearer_auth(token)
            .json(&json!({"message": message, "is_action": false}))
            .send()
            .await
            .map_err(|e| AppError::bad_gateway(format!("发送聊天失败: {e}")))?;
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("解析聊天响应失败: {e}")))?;
        if !status.is_success() {
            return Err(AppError::bad_request(format!("发送聊天失败: {body}")));
        }
        Ok(body)
    }

    /// 拉聊天记录（用用户自己的 token）
    pub async fn chat_messages(
        &self,
        osu_id: &str,
        channel_id: i64,
        after: Option<i64>,
    ) -> Result<Value, AppError> {
        let token = self.ensure_token(osu_id).await?;
        let mut url = format!("{OSU_API_BASE}/chat/channels/{channel_id}/messages");
        if let Some(after) = after {
            url.push_str(&format!("?after={after}"));
        }
        let resp = self
            .http
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| AppError::bad_gateway(format!("拉取聊天失败: {e}")))?;
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("解析聊天响应失败: {e}")))?;
        if !status.is_success() {
            return Err(AppError::bad_request(format!("拉取聊天失败: {body}")));
        }
        Ok(body)
    }

    /// 用户查询（用用户自己的 token，/api/v2/users/{lookup}/osu，lookup 可为 ID 或用户名）
    pub async fn user(&self, osu_id: &str, lookup: &str) -> Result<Value, AppError> {
        let token = self.ensure_token(osu_id).await?;
        let resp = self
            .http
            .get(format!(
                "{OSU_API_BASE}/users/{}/osu",
                crate::auth::url_encode(lookup)
            ))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| AppError::bad_gateway(format!("查询用户失败: {e}")))?;
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("解析用户响应失败: {e}")))?;
        if !status.is_success() {
            return Err(AppError::bad_request(format!("查询用户失败: {body}")));
        }
        Ok(body)
    }

    /// beatmap 查询（client_credentials 即可，无需裁判 token）
    pub async fn beatmap(&self, beatmap_id: i64) -> Result<Value, AppError> {
        let token = self.osu.access_token().await?;
        let resp = self
            .http
            .get(format!("{OSU_API_BASE}/beatmaps/{beatmap_id}"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| AppError::bad_gateway(format!("查询 beatmap 失败: {e}")))?;
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("解析 beatmap 响应失败: {e}")))?;
        if !status.is_success() {
            return Err(AppError::bad_request(format!("查询 beatmap 失败: {body}")));
        }
        Ok(body)
    }
}

// ============ 连接泵：重连 + 事件转发 ============

async fn pump(svc: Arc<RefereeService>, osu_id: String) {
    let mut attempts: u32 = 0;
    loop {
        let Some(session) = svc.get_session(&osu_id).await else {
            return;
        };
        if !session.intent.load(Ordering::SeqCst) {
            return;
        }
        session.set_state("connecting", None);
        match connect_once(&svc, &osu_id, &session.events).await {
            Ok((conn, reader)) => {
                attempts = 0;
                {
                    let mut guard = session.conn.write().await;
                    *guard = Some(conn);
                }
                session.set_state("connected", None);
                tracing::info!("referee hub 已连接: osu_id={osu_id}");
                reader.await.ok();
                {
                    let mut guard = session.conn.write().await;
                    *guard = None;
                }
                if !session.intent.load(Ordering::SeqCst) {
                    session.set_state("disconnected", None);
                    return;
                }
                tracing::warn!("referee hub 连接断开，准备重连: osu_id={osu_id}");
                session.set_state("connecting", Some("连接断开，正在重连…".to_string()));
            }
            Err(e) => {
                attempts += 1;
                tracing::warn!("referee hub 连接失败(第 {attempts} 次): {e}");
                if attempts > 10 {
                    session.intent.store(false, Ordering::SeqCst);
                    session.set_state("disconnected", Some(e));
                    return;
                }
                session.set_state("connecting", Some(e));
            }
        }
        let delay = (1u64 << attempts.min(4)).min(16);
        tokio::time::sleep(Duration::from_secs(delay)).await;
    }
}

async fn connect_once(
    svc: &RefereeService,
    osu_id: &str,
    events: &broadcast::Sender<(String, Value)>,
) -> Result<(Arc<SignalRConnection>, tokio::task::JoinHandle<()>), String> {
    let token = svc.ensure_token(osu_id).await.map_err(|e| e.message)?;
    signalr::connect(HUB_BASE, HUB_PATH, &token, events.clone()).await
}
