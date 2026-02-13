use crate::middleware::auth::AuthUser;
use axum::{
    Json, Router,
    extract::{Query, State},
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
        .route("/gu", get(gu_auth))
        .route("/callback/gu", get(gu_callback))
        .route("/refresh", get(refresh_token))
        .route("/logout", get(logout))
}

#[derive(Debug, Deserialize)]
struct OsuCallbackQuery {
    code: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct RefreshTokenQuery {
    server: Option<String>,
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
    path = "/api/auth/gu",
    responses(
        (status = 302, description = "重定向到GU授权页面"),
    ),
    tag = "auth"
)]
async fn gu_auth() -> Redirect {
    let client_id = env::var("GU_CLIENT_ID").expect("GU_CLIENT_ID not set");
    let redirect_uri = env::var("GU_REDIRECT_URI").expect("GU_REDIRECT_URI not set");
    let state = Uuid::new_v4().to_string();

    let auth_url = format!(
        "https://lazer-api.g0v0.top/oauth/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify%20public&state={}",
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
        Some(existing) => {
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
            let updated_user = user_service.update(existing.id, update_request).await?;

            // 保存 OSU refresh_token
            user_service
                .update_osu_refresh_token(updated_user.id, token_response.refresh_token.clone())
                .await?;
            updated_user
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
            let new_user = user_service.create(user_data.id, create_request).await?;

            // 保存 OSU refresh_token
            user_service
                .update_osu_refresh_token(new_user.id, token_response.refresh_token.clone())
                .await?;
            new_user
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
    params(
        ("server" = Option<String>, Query, description = "服务器类型，可选值：osu, gu，默认为osu"),
    ),
    responses(
        (status = 200, description = "令牌刷新成功"),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权或refresh_token过期"),
        (status = 500, description = "内部服务器错误"),
    ),
    tag = "auth"
)]
async fn refresh_token(
    auth_user: AuthUser,
    Query(params): Query<RefreshTokenQuery>,
    State(state): State<ServiceState>,
) -> Result<Json<ApiResponse<AuthResponse>>> {
    let server = params.server.unwrap_or_else(|| "osu".to_string());

    if server != "osu" && server != "gu" {
        return Err(crate::error::AppError::ValidationError(
            "server参数必须是'osu'或'gu'".into(),
        ));
    }

    let user_service = UserService::new(state.db);

    // 根据服务器类型获取对应的refresh_token
    let refresh_token = match server.as_str() {
        "osu" => user_service.get_osu_refresh_token(auth_user.id).await?,
        "gu" => user_service.get_gu_refresh_token(auth_user.id).await?,
        _ => unreachable!(),
    };

    let refresh_token = refresh_token.ok_or_else(|| {
        crate::error::AppError::Unauthorized(format!("{} refresh_token不存在", server))
    })?;

    // 根据服务器类型调用相应的API刷新令牌
    let (client_id, client_secret, token_url, api_url) = match server.as_str() {
        "osu" => (
            env::var("OSU_CLIENT_ID").expect("OSU_CLIENT_ID not set"),
            env::var("OSU_CLIENT_SECRET").expect("OSU_CLIENT_SECRET not set"),
            "https://osu.ppy.sh/oauth/token",
            "https://osu.ppy.sh/api/v2/me",
        ),
        "gu" => (
            env::var("GU_CLIENT_ID").expect("GU_CLIENT_ID not set"),
            env::var("GU_CLIENT_SECRET").expect("GU_CLIENT_SECRET not set"),
            "https://lazer-api.g0v0.top/oauth/token",
            "https://lazer-api.g0v0.top/api/v2/me",
        ),
        _ => unreachable!(),
    };

    let client = Client::new();

    // 使用refresh_token获取新的访问令牌
    let token_response = client
        .post(token_url)
        .json(&serde_json::json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "refresh_token": refresh_token,
            "grant_type": "refresh_token"
        }))
        .send()
        .await
        .map_err(|e| {
            // 如果refresh_token过期，返回401提示重新登录
            if e.status() == Some(reqwest::StatusCode::UNAUTHORIZED) {
                crate::error::AppError::Unauthorized(format!(
                    "{} refresh_token已过期，请重新登录",
                    server
                ))
            } else {
                crate::error::AppError::ExternalApiError(format!("Failed to refresh token: {}", e))
            }
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
        .get(api_url)
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

    // 更新用户信息
    let update_request = crate::dto::user::UpdateUserRequest {
        username: Some(user_data.username.clone()),
        avatar_url: user_data.avatar_url,
        pp: user_data.statistics.as_ref().and_then(|s| s.pp),
        global_rank: user_data.statistics.as_ref().and_then(|s| s.global_rank),
        country: user_data.country.as_ref().map(|c| c.code.clone()),
        country_rank: user_data.statistics.as_ref().and_then(|s| s.country_rank),
        approved: None,
        season: None,
    };

    let user = user_service.update(auth_user.id, update_request).await?;

    // 更新数据库中的refresh_token
    match server.as_str() {
        "osu" => {
            user_service
                .update_osu_refresh_token(auth_user.id, token_response.refresh_token.clone())
                .await?;
        }
        "gu" => {
            user_service
                .update_gu_refresh_token(auth_user.id, token_response.refresh_token.clone())
                .await?;
        }
        _ => unreachable!(),
    }

    // 生成新的JWT令牌
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
    path = "/api/auth/logout",
    responses(
        (status = 200, description = "登出成功"),
        (status = 401, description = "未授权"),
    ),
    tag = "auth"
)]
async fn logout(
    auth_user: AuthUser,
    State(state): State<ServiceState>,
) -> Result<Json<ApiResponse<()>>> {
    // 清除用户的refresh_token
    let user_service = UserService::new(state.db);

    // 清除所有refresh_token（OSU和GU）
    user_service.clear_all_refresh_tokens(auth_user.id).await?;

    // 返回成功消息
    Ok(Json(ApiResponse::success_with_message(
        (),
        "登出成功，已清除OSU和GU refresh_token",
    )))
}

#[utoipa::path(
    get,
    path = "/api/auth/callback/gu",
    params(
        ("code" = String, Query, description = "GU OAuth回调代码"),
        ("state" = String, Query, description = "状态参数"),
    ),
    responses(
        (status = 200, description = "OAuth回调成功"),
        (status = 400, description = "请求参数错误"),
        (status = 500, description = "内部服务器错误"),
    ),
    tag = "auth"
)]
async fn gu_callback(
    Query(params): Query<OsuCallbackQuery>,
    State(state): State<ServiceState>,
) -> Result<Json<ApiResponse<AuthResponse>>> {
    let client_id = env::var("GU_CLIENT_ID").expect("GU_CLIENT_ID not set");
    let client_secret = env::var("GU_CLIENT_SECRET").expect("GU_CLIENT_SECRET not set");
    let redirect_uri = env::var("GU_REDIRECT_URI").expect("GU_REDIRECT_URI not set");

    // 获取访问令牌
    let client = Client::new();
    let token_response = client
        .post("https://lazer-api.g0v0.top/oauth/token")
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
        .get("https://lazer-api.g0v0.top/api/v2/me")
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

    // 提前复制需要的值，避免移动后借用
    let user_id = user_data.id.clone();
    let username = user_data.username.clone();

    let user = match existing_user {
        Some(existing) => {
            // 更新现有用户，包括GU服务器信息
            let update_request = crate::dto::user::UpdateUserRequest {
                username: Some(username.clone()),
                avatar_url: user_data.avatar_url,
                pp: user_data.statistics.as_ref().and_then(|s| s.pp),
                global_rank: user_data.statistics.as_ref().and_then(|s| s.global_rank),
                country: user_data.country.as_ref().map(|c| c.code.clone()),
                country_rank: user_data.statistics.as_ref().and_then(|s| s.country_rank),
                approved: None,
                season: None,
            };
            let updated_user = user_service.update(existing.id, update_request).await?;

            // 更新用户的gu_server_user_id和gu_server_username
            let gu_user_id = user_id.parse::<i32>().unwrap_or(0);
            let updated_user_with_gu_info = user_service
                .update_gu_server_info(updated_user.id, gu_user_id, username.clone())
                .await?;

            // 保存 GU refresh_token
            user_service
                .update_gu_refresh_token(
                    updated_user_with_gu_info.id,
                    token_response.refresh_token.clone(),
                )
                .await?;
            updated_user_with_gu_info
        }
        None => {
            // 创建新用户
            let create_request = CreateUserRequest {
                username: username.clone(),
                avatar_url: user_data.avatar_url,
                pp: user_data.statistics.as_ref().and_then(|s| s.pp),
                global_rank: user_data.statistics.as_ref().and_then(|s| s.global_rank),
                country: user_data.country.as_ref().map(|c| c.code.clone()),
                country_rank: user_data.statistics.as_ref().and_then(|s| s.country_rank),
            };
            let new_user = user_service.create(user_id.clone(), create_request).await?;

            // 设置用户的gu_server_user_id和gu_server_username
            let gu_user_id = user_id.parse::<i32>().unwrap_or(0);
            let new_user_with_gu_info = user_service
                .update_gu_server_info(new_user.id, gu_user_id, username.clone())
                .await?;

            // 保存 GU refresh_token
            user_service
                .update_gu_refresh_token(
                    new_user_with_gu_info.id,
                    token_response.refresh_token.clone(),
                )
                .await?;
            new_user_with_gu_info
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
