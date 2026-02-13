use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
};
use utoipa::ToSchema;

use crate::{
    dto::{
        ApiResponse, PaginatedResponse,
        user::{
            CreateUserRequest, UpdateApprovedRequest, UpdateRegistrationStatusRequest,
            UpdateSeasonRequest, UpdateUserGroupRequest, UpdateUserRequest, UserQueryParams,
            UserResponse,
        },
    },
    error::Result,
    middleware::auth::{AuthUser, require_admin},
    service::{ServiceState, user::UserService},
};

pub fn routes() -> Router<ServiceState> {
    Router::new()
        .route("/", get(get_users))
        .route("/", post(create_user))
        .route("/me", get(get_current_user))
        .route("/{osu_id}", get(get_user))
        .route("/{osu_id}", put(update_user))
        .route("/{osu_id}", delete(delete_user))
        .route("/{osu_id}/user-group", put(update_user_group))
        .route(
            "/{osu_id}/registration-status",
            put(update_registration_status),
        )
        .route("/{osu_id}/season", put(update_season))
        .route("/{osu_id}/approve", put(update_approved))
}

#[utoipa::path(
    get,
    path = "/api/users",
    params(UserQueryParams),
    responses(
        (status = 200, description = "获取osu玩家列表成功", body = PaginatedUserResponse),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
    ),
    tag = "users"
)]
async fn get_users(
    State(state): State<ServiceState>,
    Query(params): Query<UserQueryParams>,
) -> Result<Json<ApiResponse<PaginatedResponse<UserResponse>>>> {
    let service = UserService::new(state.db);
    let (users, total) = service.find_all(params).await?;

    let response = PaginatedResponse::new(
        users.into_iter().map(UserResponse::from).collect(),
        total,
        1,
        20,
    );

    Ok(Json(ApiResponse::success(response)))
}

#[utoipa::path(
    get,
    path = "/api/users/{osu_id}",
    params(
        ("osu_id" = String, Path, description = "osuID")
    ),
    responses(
        (status = 200, description = "获取osu玩家成功", body = UserResponse),
        (status = 404, description = "osu玩家不存在"),
    ),
    tag = "users"
)]
async fn get_user(
    State(state): State<ServiceState>,
    Path(osu_id): Path<String>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    let service = UserService::new(state.db);
    let user = service
        .find_by_osu_id(&osu_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("osu玩家 {} 不存在", osu_id)))?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    get,
    path = "/api/users/me",
    responses(
        (status = 200, description = "获取当前玩家信息成功", body = UserResponse),
        (status = 401, description = "未授权"),
        (status = 404, description = "玩家不存在"),
    ),
    tag = "users"
)]
async fn get_current_user(
    State(state): State<ServiceState>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<UserResponse>>> {
    let service = UserService::new(state.db);
    let user = service
        .find_by_osu_id(&auth_user.osu_id)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::NotFound(format!("玩家 {} 不存在", auth_user.osu_id))
        })?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    post,
    path = "/api/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "创建osu玩家成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
    ),
    tag = "users"
)]
async fn create_user() -> Result<Json<ApiResponse<UserResponse>>> {
    // osu玩家创建将通过OAuth回调处理
    Err(crate::error::AppError::BusinessError(
        "请通过OAuth注册osu玩家".into(),
    ))
}

#[utoipa::path(
    put,
    path = "/api/users/{osu_id}",
    params(
        ("osu_id" = String, Path, description = "osu玩家ID")
    ),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "更新osu玩家成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "osu玩家不存在"),
    ),
    tag = "users"
)]
#[axum::debug_handler]
async fn update_user(
    State(state): State<ServiceState>,
    Path(osu_id): Path<String>,
    auth_user: AuthUser,
    Json(data): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    let service = UserService::new(state.db);

    // 获取要更新的osu玩家信息
    let target_user = service
        .find_by_osu_id(&osu_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("osu玩家 {} 不存在", osu_id)))?;

    // 检查权限：管理员或osu玩家本人可以更新
    let is_admin = auth_user.user_group == "admin";
    let is_self = auth_user.osu_id == target_user.osu_id;

    if !is_admin && !is_self {
        return Err(crate::error::AppError::Forbidden("权限错误".into()));
    }

    let user = service.update(target_user.id, data).await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    put,
    path = "/api/users/{osu_id}/user-group",
    params(
        ("osu_id" = String, Path, description = "osuID")
    ),
    request_body = UpdateUserGroupRequest,
    responses(
        (status = 200, description = "更新osu玩家组成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "osu玩家不存在"),
    ),
    tag = "users"
)]
#[axum::debug_handler]
async fn update_user_group(
    State(state): State<ServiceState>,
    Path(osu_id): Path<String>,
    user: AuthUser,
    Json(data): Json<UpdateUserGroupRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    // 检查是否为管理员
    require_admin(&user)?;

    let service = UserService::new(state.db);
    let target_user = service
        .find_by_osu_id(&osu_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("osu玩家 {} 不存在", osu_id)))?;

    let user = service.update_user_group(target_user.id, data).await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    delete,
    path = "/api/users/{osu_id}",
    params(
        ("osu_id" = String, Path, description = "osuID")
    ),
    responses(
        (status = 200, description = "删除osu玩家成功"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "osu玩家不存在"),
    ),
    tag = "users"
)]
async fn delete_user(
    State(state): State<ServiceState>,
    Path(osu_id): Path<String>,
    user: AuthUser,
) -> Result<Json<ApiResponse<()>>> {
    // 检查是否为管理员
    require_admin(&user)?;

    let service = UserService::new(state.db);
    let target_user = service
        .find_by_osu_id(&osu_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("osu玩家 {} 不存在", osu_id)))?;

    service.delete(target_user.id).await?;

    Ok(Json(ApiResponse::success_with_message(
        (),
        "osu玩家删除成功",
    )))
}

#[utoipa::path(
    put,
    path = "/api/users/{osu_id}/registration-status",
    params(
        ("osu_id" = String, Path, description = "osuID")
    ),
    request_body = UpdateRegistrationStatusRequest,
    responses(
        (status = 200, description = "更新注册状态成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 404, description = "osu玩家不存在"),
    ),
    tag = "users"
)]
async fn update_registration_status(
    State(state): State<ServiceState>,
    Path(osu_id): Path<String>,
    Json(data): Json<UpdateRegistrationStatusRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    let service = UserService::new(state.db);
    let target_user = service
        .find_by_osu_id(&osu_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("osu玩家 {} 不存在", osu_id)))?;

    let user = service
        .update_registration_status(target_user.id, data)
        .await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    put,
    path = "/api/users/{osu_id}/season",
    params(
        ("osu_id" = String, Path, description = "osuID")
    ),
    request_body = UpdateSeasonRequest,
    responses(
        (status = 200, description = "更新赛季成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 404, description = "osu玩家不存在"),
    ),
    tag = "users"
)]
async fn update_season(
    State(state): State<ServiceState>,
    Path(osu_id): Path<String>,
    Json(data): Json<UpdateSeasonRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    let service = UserService::new(state.db);
    let target_user = service
        .find_by_osu_id(&osu_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("osu玩家 {} 不存在", osu_id)))?;

    let user = service.update_season(target_user.id, data).await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    put,
    path = "/api/users/{osu_id}/approve",
    params(
        ("osu_id" = String, Path, description = "osuID")
    ),
    request_body = UpdateApprovedRequest,
    responses(
        (status = 200, description = "更新审批状态成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "osu玩家不存在"),
    ),
    tag = "users"
)]
#[axum::debug_handler]
async fn update_approved(
    State(state): State<ServiceState>,
    Path(osu_id): Path<String>,
    user: AuthUser,
    Json(data): Json<UpdateApprovedRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    // 检查是否为管理员
    require_admin(&user)?;

    let service = UserService::new(state.db);
    let target_user = service
        .find_by_osu_id(&osu_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("osu玩家 {} 不存在", osu_id)))?;

    let user = service.update_approved(target_user.id, data, true).await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[derive(ToSchema)]
pub struct PaginatedUserResponse {
    items: Vec<UserResponse>,
    total: u64,
    page: u64,
    page_size: u64,
    total_pages: u64,
}
