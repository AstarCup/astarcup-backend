use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::Redirect,
    routing::get,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;

use crate::{
    dto::{
        ApiResponse,
        user::{CreateUserRequest, OsuUserData},
    },
    error::Result,
    service::{ServiceState, user::UserService},
};

pub fn routes() -> Router<ServiceState> {
    Router::new()
        .route("/osu", get(osu_auth))
        .route("/callback/osu", get(osu_callback))
        .route("/refresh", get(refresh_token))
        .route("/logout", get(logout))
}

#[derive(Debug, Deserialize)]
struct OsuCallbackQuery {
    code: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct OsuTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u32,
    refresh_token: String,
    scope: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AuthResponse {
    access_token: String,
    refresh_token: String,
    user: crate::dto::user::UserResponse,
}

#[utoipa::path(
    get,
    path = "/api/auth/osu",
    responses(
        (status = 302, description = "重定向到OSU授权页面"),
    ),
    tag = "auth"
)]
async fn osu_auth() -> Redirect {
    let client_id = env::var("OSU_CLIENT_ID").expect("OSU_CLIENT_ID not set");
    let redirect_uri = env::var("OSU_REDIRECT_URI").expect("OSU_REDIRECT_URI not set");
    let state = Uuid::new_v4().to_string();

    let auth_url = format!(
        "https://osu.ppy.sh/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify%20public&state={}",
        client_id,
        urlencoding::encode(&redirect_uri),
        state
    );

    Redirect::to(&auth_url)
}

#[utoipa::path(
    get,
    path = "/api/auth/callback/osu",
    params(
        ("code" = String, Query, description = "OSU OAuth回调代码"),
        ("state" = String, Query, description = "状态参数"),
    ),
    responses(
        (status = 200, description = "OAuth回调成功"),
        (status = 400, description = "请求参数错误"),
        (status = 500, description = "内部服务器错误"),
    ),
    tag = "auth"
)]
async fn osu_callback(
    Query(params): Query<OsuCallbackQuery>,
    State(state): State<ServiceState>,
) -> Result<Json<ApiResponse<AuthResponse>>> {
    let client_id = env::var("OSU_CLIENT_ID").expect("OSU_CLIENT_ID not set");
    let client_secret = env::var("OSU_CLIENT_SECRET").expect("OSU_CLIENT_SECRET not set");
    let redirect_uri = env::var("OSU_REDIRECT_URI").expect("OSU_REDIRECT_URI not set");

    // 获取访问令牌
    let client = Client::new();
    let token_response = client
        .post("https://osu.ppy.sh/oauth/token")
        .json(&serde_json::json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "code": params.code,
            "grant_type": "authorization_code",
            "redirect_uri": redirect_uri
        }))
        .send()
        .await
        .map_err(|e| {
            crate::error::AppError::ExternalApiError(format!("Failed to get token: {}", e))
        })?
        .json::<OsuTokenResponse>()
        .await
        .map_err(|e| {
            crate::error::AppError::ExternalApiError(format!(
                "Failed to parse token response: {}",
                e
            ))
        })?;

    // 获取用户信息
    let user_data = client
        .get("https://osu.ppy.sh/api/v2/me")
        .header(
            "Authorization",
            format!("Bearer {}", token_response.access_token),
        )
        .send()
        .await
        .map_err(|e| {
            crate::error::AppError::ExternalApiError(format!("Failed to get user info: {}", e))
        })?
        .json::<OsuUserData>()
        .await
        .map_err(|e| {
            crate::error::AppError::ExternalApiError(format!("Failed to parse user info: {}", e))
        })?;

    // 处理用户数据
    let user_service = UserService::new(state.db);
    let existing_user = user_service.find_by_osu_id(&user_data.id).await?;

    let user = match existing_user {
        Some(mut existing) => {
            // 更新现有用户
            let update_request = crate::dto::user::UpdateUserRequest {
                username: Some(user_data.username),
                avatar_url: user_data.avatar_url,
                pp: user_data.statistics.as_ref().and_then(|s| s.pp),
                global_rank: user_data.statistics.as_ref().and_then(|s| s.global_rank),
                country: user_data.country.as_ref().map(|c| c.code.clone()),
                country_rank: user_data.statistics.as_ref().and_then(|s| s.country_rank),
                approved: None,
                season: None,
            };
            user_service.update(existing.id, update_request).await?
        }
        None => {
            // 创建新用户
            let create_request = CreateUserRequest {
                username: user_data.username,
                avatar_url: user_data.avatar_url,
                pp: user_data.statistics.as_ref().and_then(|s| s.pp),
                global_rank: user_data.statistics.as_ref().and_then(|s| s.global_rank),
                country: user_data.country.as_ref().map(|c| c.code.clone()),
                country_rank: user_data.statistics.as_ref().and_then(|s| s.country_rank),
            };
            user_service.create(user_data.id, create_request).await?
        }
    };

    // 生成JWT令牌
    let osu_id = user.osu_id.clone();
    let user_group_str = match user.user_group {
        crate::entity::sea_orm_active_enums::UserGroup::Player => "player",
        crate::entity::sea_orm_active_enums::UserGroup::Admin => "admin",
    };
    let jwt_token =
        crate::utils::jwt::generate_token(osu_id, user_group_str.to_string()).map_err(|e| {
            crate::error::AppError::InternalError(format!("Failed to generate token: {}", e))
        })?;

    let auth_response = AuthResponse {
        access_token: jwt_token,
        refresh_token: token_response.refresh_token,
        user: crate::dto::user::UserResponse::from(user),
    };

    Ok(Json(ApiResponse::success(auth_response)))
}

#[utoipa::path(
    get,
    path = "/api/auth/refresh",
    responses(
        (status = 200, description = "令牌刷新成功"),
        (status = 401, description = "未授权"),
        (status = 500, description = "内部服务器错误"),
    ),
    tag = "auth"
)]
async fn refresh_token() -> Result<Json<ApiResponse<AuthResponse>>> {
    // 实现令牌刷新逻辑
    // 这里需要从请求中获取refresh_token，然后调用OSU API刷新令牌
    // 暂时返回未实现
    Err(crate::error::AppError::NotImplemented(
        "Token refresh not implemented".into(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/auth/logout",
    responses(
        (status = 200, description = "登出成功"),
    ),
    tag = "auth"
)]
async fn logout() -> Result<Json<ApiResponse<()>>> {
    // 实现登出逻辑
    // 这里需要清除客户端的令牌
    Ok(Json(ApiResponse::success_with_message((), "登出成功")))
}
