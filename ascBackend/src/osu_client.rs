// osu! API v2 客户端：client_credentials 令牌缓存（提前 60 秒刷新）+ 代理请求
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::error::AppError;

const OSU_API_BASE: &str = "https://osu.ppy.sh/api/v2";
const TOKEN_URL: &str = "https://osu.ppy.sh/oauth/token";

struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

struct Inner {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    token: Mutex<Option<CachedToken>>,
}

#[derive(Clone)]
pub struct OsuClient {
    inner: Arc<Inner>,
}

impl OsuClient {
    pub fn new(http: reqwest::Client, cfg: &Config) -> Self {
        OsuClient {
            inner: Arc::new(Inner {
                http,
                client_id: cfg.osu_client_id.clone(),
                client_secret: cfg.osu_client_secret.clone(),
                token: Mutex::new(None),
            }),
        }
    }

    /// 获取 client_credentials access token（带缓存，过期前 60s 刷新）
    pub async fn access_token(&self) -> Result<String, AppError> {
        let mut guard = self.inner.token.lock().await;
        if let Some(t) = guard.as_ref() {
            if t.expires_at > Instant::now() {
                return Ok(t.access_token.clone());
            }
        }
        let resp = self
            .inner
            .http
            .post(TOKEN_URL)
            .json(&json!({
                "grant_type": "client_credentials",
                "client_id": self.inner.client_id,
                "client_secret": self.inner.client_secret,
                "scope": "public",
            }))
            .send()
            .await
            .map_err(|e| AppError::internal(format!("获取 osu! API token 失败: {e}")))?;
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("解析 osu! token 响应失败: {e}")))?;
        if !status.is_success() {
            return Err(AppError::internal(format!(
                "osu! client_credentials 失败: {body}"
            )));
        }
        let access_token = body["access_token"]
            .as_str()
            .ok_or_else(|| AppError::internal("osu! token 响应缺少 access_token"))?
            .to_string();
        let expires_in = body["expires_in"].as_i64().unwrap_or(3600);
        *guard = Some(CachedToken {
            access_token: access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs((expires_in - 60).max(60) as u64),
        });
        Ok(access_token)
    }

    /// GET 请求 osu! API v2，返回 JSON
    pub async fn get_json(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, AppError> {
        let token = self.access_token().await?;
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{OSU_API_BASE}{path}")
        };
        let mut req = self.inner.http.get(&url).bearer_auth(token);
        for (k, v) in query {
            req = req.query(&[(k, v)]);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::internal(format!("请求 osu! API 失败: {e}")))?;
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("解析 osu! API 响应失败: {e}")))?;
        if !status.is_success() {
            let code = axum::http::StatusCode::from_u16(status.as_u16())
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
            return Err(AppError::new(code, format!("osu! API 错误: {body}")));
        }
        Ok(body)
    }

    /// GET 请求返回原始字节（用于下载 beatmap 文件等）
    #[allow(dead_code)]
    pub async fn get_bytes(&self, url: &str, query: &[(&str, &str)]) -> Result<Vec<u8>, AppError> {
        let token = self.access_token().await?;
        let mut req = self.inner.http.get(url).bearer_auth(token);
        for (k, v) in query {
            req = req.query(&[(k, v)]);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::internal(format!("下载 osu! 资源失败: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            let code = axum::http::StatusCode::from_u16(status.as_u16())
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
            return Err(AppError::new(
                code,
                format!("osu! 资源下载失败: HTTP {}", status.as_u16()),
            ));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| AppError::internal(format!("读取下载内容失败: {e}")))?;
        Ok(bytes.to_vec())
    }
}
