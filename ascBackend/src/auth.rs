// 统一用户认证：JWT(HS256) HttpOnly cookie + osu! OAuth2 登录流程
// 会话 cookie 名保持与原版一致：astra_session
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use chrono::{Duration, Utc};
use diesel::prelude::*;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::db::{DbPool, run_db};
use crate::enums::{RegistrationStatus, UserGroup};
use crate::error::AppError;
use crate::models::User;
use crate::schema::users;
use crate::state::AppState;

pub const SESSION_COOKIE: &str = "astra_session";

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    #[serde(rename = "osuId")]
    pub osu_id: String,
    pub username: String,
    #[serde(rename = "avatar_url", default)]
    pub avatar_url: String,
    pub exp: usize,
}

// ============ JWT 签发与校验 ============

pub fn create_token(
    cfg: &Config,
    osu_id: &str,
    username: &str,
    avatar_url: Option<&str>,
) -> Result<String, AppError> {
    let exp = (Utc::now() + Duration::days(cfg.jwt_expire_days)).timestamp() as usize;
    let claims = Claims {
        osu_id: osu_id.to_string(),
        username: username.to_string(),
        avatar_url: avatar_url.unwrap_or("").to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::internal(format!("签发会话失败: {e}")))
}

pub fn verify_token(cfg: &Config, token: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|d| d.claims)
    .map_err(|_| AppError::unauthorized("会话无效或已过期"))
}

// ============ Cookie 构造 ============

/// 登录成功后下发的会话 cookie（HttpOnly）
pub fn session_cookie(cfg: &Config, token: &str) -> String {
    let max_age = cfg.jwt_expire_days * 24 * 3600;
    let mut cookie =
        format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}");
    if cfg.cookie_secure {
        cookie.push_str("; Secure");
    }
    if !cfg.cookie_domain.is_empty() {
        cookie.push_str(&format!("; Domain={}", cfg.cookie_domain));
    }
    cookie
}

/// 注销用的清空 cookie
pub fn clear_session_cookie(cfg: &Config) -> String {
    let mut cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if cfg.cookie_secure {
        cookie.push_str("; Secure");
    }
    if !cfg.cookie_domain.is_empty() {
        cookie.push_str(&format!("; Domain={}", cfg.cookie_domain));
    }
    cookie
}

fn read_cookie(parts: &Parts, name: &str) -> Option<String> {
    let raw = parts
        .headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?;
    for pair in raw.split(';') {
        let mut it = pair.trim().splitn(2, '=');
        let k = it.next()?;
        let v = it.next()?;
        if k == name {
            return Some(v.to_string());
        }
    }
    None
}

// ============ 请求提取器 ============

/// 已登录用户（从 JWT cookie 解出）
#[derive(Debug, Clone)]
pub struct SessionUser {
    pub osu_id: String,
    pub username: String,
    pub avatar_url: String,
}

impl FromRequestParts<AppState> for SessionUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token =
            read_cookie(parts, SESSION_COOKIE).ok_or_else(|| AppError::unauthorized("未登录"))?;
        let claims = verify_token(&state.config, &token)?;
        Ok(SessionUser {
            osu_id: claims.osu_id,
            username: claims.username,
            avatar_url: claims.avatar_url,
        })
    }
}

/// 支持 Option<SessionUser> 提取（未登录时得到 None）
impl axum::extract::OptionalFromRequestParts<AppState> for SessionUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Option<Self>, Self::Rejection> {
        match <Self as FromRequestParts<AppState>>::from_request_parts(parts, state).await {
            Ok(u) => Ok(Some(u)),
            Err(e) if e.status == axum::http::StatusCode::UNAUTHORIZED => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// 管理员用户（JWT 校验后再查库确认 userGroup = admin）
#[allow(dead_code)]
pub struct AdminUser {
    pub osu_id: String,
    pub username: String,
    pub user: User,
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token =
            read_cookie(parts, SESSION_COOKIE).ok_or_else(|| AppError::unauthorized("未登录"))?;
        let claims = verify_token(&state.config, &token)?;
        let osu_id = claims.osu_id.clone();
        let pool = state.pool.clone();
        let user = run_db(&pool, move |conn| {
            users::table
                .filter(users::osu_id.eq(&osu_id))
                .first::<User>(conn)
                .optional()
                .map_err(AppError::from)?
                .ok_or_else(|| AppError::unauthorized("用户不存在"))
        })
        .await?;
        if user.user_group != UserGroup::Admin {
            return Err(AppError::forbidden("需要管理员权限"));
        }
        Ok(AdminUser {
            osu_id: user.osu_id.clone(),
            username: user.username.clone(),
            user,
        })
    }
}

/// 支持 Option<AdminUser> 提取（未登录或未授权时得到 None）
impl axum::extract::OptionalFromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Option<Self>, Self::Rejection> {
        match <Self as FromRequestParts<AppState>>::from_request_parts(parts, state).await {
            Ok(u) => Ok(Some(u)),
            Err(e) if e.status == axum::http::StatusCode::UNAUTHORIZED => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ============ osu! OAuth2（authorization_code 登录） ============

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct OsuToken {
    pub access_token: String,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

/// 生成 osu! 授权跳转 URL；state 用于 OAuth 完成后回跳到网站指定页面
pub fn authorize_url(cfg: &Config, state: Option<&str>) -> String {
    let mut url = format!(
        "https://osu.ppy.sh/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify",
        url_encode(&cfg.osu_client_id),
        url_encode(&cfg.osu_redirect_uri),
    );
    if let Some(state) = state {
        url.push_str("&state=");
        url.push_str(&url_encode(state));
    }
    url
}

/// 用授权 code 换取 access token
pub async fn exchange_code(
    cfg: &Config,
    http: &reqwest::Client,
    code: &str,
) -> Result<OsuToken, AppError> {
    let resp = http
        .post("https://osu.ppy.sh/oauth/token")
        .form(&[
            ("client_id", cfg.osu_client_id.as_str()),
            ("client_secret", cfg.osu_client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", cfg.osu_redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|e| AppError::internal(format!("请求 osu! 授权服务器失败: {e}")))?;
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| AppError::internal(format!("解析 osu! 授权响应失败: {e}")))?;
    if !status.is_success() {
        return Err(AppError::bad_request(format!("osu! 授权失败: {body}")));
    }
    serde_json::from_value(body)
        .map_err(|e| AppError::internal(format!("osu! 授权响应格式错误: {e}")))
}

/// 用 access token 获取 osu! 当前用户信息（/api/v2/me）
pub async fn fetch_osu_me(http: &reqwest::Client, access_token: &str) -> Result<Value, AppError> {
    let resp = http
        .get("https://osu.ppy.sh/api/v2/me")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::internal(format!("请求 osu! 用户信息失败: {e}")))?;
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| AppError::internal(format!("解析 osu! 用户信息失败: {e}")))?;
    if !status.is_success() {
        return Err(AppError::bad_request(format!(
            "获取 osu! 用户信息失败: {body}"
        )));
    }
    Ok(body)
}

/// 按 osu! /me 信息创建或更新本地用户。
/// 已存在：只更新基础资料（用户名/头像/pp/rank/国家/cover），不动 userGroup、season、registrationStatus；
/// 不存在：新建 player 用户，registration_status = not_registered。
pub async fn upsert_osu_user(pool: &DbPool, me: &Value) -> Result<User, AppError> {
    let osu_id = me["id"].as_i64().map(|v| v.to_string()).unwrap_or_default();
    if osu_id.is_empty() {
        return Err(AppError::internal("osu! 用户信息缺少 id"));
    }
    let username = me["username"].as_str().unwrap_or("").to_string();
    let avatar_url = me["avatar_url"].as_str().map(|s| s.to_string());
    let pp = me["statistics"]["pp"].as_f64();
    let global_rank = me["statistics"]["global_rank"].as_i64().map(|v| v as i32);
    let country_rank = me["statistics"]["country_rank"].as_i64().map(|v| v as i32);
    let country = me["country_code"].as_str().unwrap_or("").to_string();
    let cover_custom_url = me["cover"]["custom_url"].as_str().map(|s| s.to_string());
    let cover_url = me["cover"]["url"]
        .as_str()
        .or_else(|| me["cover_url"].as_str())
        .map(|s| s.to_string());
    let cover_id = me["cover"]["id"].as_str().map(|s| s.to_string());
    let now = Utc::now().naive_utc();

    let osu_id_c = osu_id.clone();
    run_db(pool, move |conn| {
        let existing: Option<User> = users::table
            .filter(users::osu_id.eq(&osu_id_c))
            .first(conn)
            .optional()
            .map_err(AppError::from)?;
        if let Some(u) = existing {
            Ok(diesel::update(users::table.find(u.id))
                .set((
                    users::username.eq(&username),
                    users::avatar_url.eq(&avatar_url),
                    users::pp.eq(pp),
                    users::global_rank.eq(global_rank),
                    users::country_rank.eq(country_rank),
                    users::country.eq(&country),
                    users::cover_custom_url.eq(&cover_custom_url),
                    users::cover_url.eq(&cover_url),
                    users::cover_id.eq(&cover_id),
                    users::updated_at.eq(now),
                ))
                .get_result::<User>(conn)
                .map_err(AppError::from)?)
        } else {
            Ok(diesel::insert_into(users::table)
                .values((
                    users::osu_id.eq(&osu_id_c),
                    users::username.eq(&username),
                    users::registered_at.eq(now),
                    users::avatar_url.eq(&avatar_url),
                    users::pp.eq(pp),
                    users::global_rank.eq(global_rank),
                    users::country_rank.eq(country_rank),
                    users::country.eq(&country),
                    users::user_group.eq(UserGroup::Player),
                    users::registration_status.eq(RegistrationStatus::NotRegistered),
                    users::cover_custom_url.eq(&cover_custom_url),
                    users::cover_url.eq(&cover_url),
                    users::cover_id.eq(&cover_id),
                    users::created_at.eq(now),
                    users::updated_at.eq(now),
                ))
                .get_result::<User>(conn)
                .map_err(AppError::from)?)
        }
    })
    .await
}

// ============ 小工具 ============

/// 极简 percent-encode（用于拼接授权 URL）
pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
