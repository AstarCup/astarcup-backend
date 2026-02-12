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
        .route("/{id}", get(get_user))
        .route("/{id}", put(update_user))
        .route("/{id}", delete(delete_user))
        .route("/{id}/user-group", put(update_user_group))
        .route("/{id}/registration-status", put(update_registration_status))
        .route("/{id}/season", put(update_season))
        .route("/{id}/approve", put(update_approved))
}

#[utoipa::path(
    get,
    path = "/api/users",
    params(UserQueryParams),
    responses(
        (status = 200, description = "获取用户列表成功", body = PaginatedUserResponse),
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
    path = "/api/users/{id}",
    params(
        ("id" = i32, Path, description = "用户ID")
    ),
    responses(
        (status = 200, description = "获取用户成功", body = UserResponse),
        (status = 404, description = "用户不存在"),
    ),
    tag = "users"
)]
async fn get_user(
    State(state): State<ServiceState>,
    Path(id): Path<i32>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    let service = UserService::new(state.db);
    let user = service.find_by_id(id).await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    post,
    path = "/api/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "创建用户成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
    ),
    tag = "users"
)]
async fn create_user() -> Result<Json<ApiResponse<UserResponse>>> {
    // 用户创建将通过OAuth回调处理
    Err(crate::error::AppError::BusinessError(
        "请通过OAuth注册用户".into(),
    ))
}

#[utoipa::path(
    put,
    path = "/api/users/{id}",
    params(
        ("id" = i32, Path, description = "用户ID")
    ),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "更新用户成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "用户不存在"),
    ),
    tag = "users"
)]
#[axum::debug_handler]
async fn update_user(
    State(state): State<ServiceState>,
    Path(id): Path<i32>,
    auth_user: AuthUser,
    Json(data): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    // 检查是否为管理员，或者是否更新自己的信息
    // 注意：这里需要从数据库获取用户的osu_id来匹配
    // 暂时简化处理，实际项目中需要查询数据库
    let is_admin = auth_user.user_group == "admin";
    let is_self = auth_user.osu_id == id.to_string(); // 这里需要根据实际情况调整

    if !is_admin && !is_self {
        return Err(crate::error::AppError::Forbidden(
            "只有管理员或用户本人可以更新用户信息".into(),
        ));
    }

    let service = UserService::new(state.db);
    let user = service.update(id, data).await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    put,
    path = "/api/users/{id}/user-group",
    params(
        ("id" = i32, Path, description = "用户ID")
    ),
    request_body = UpdateUserGroupRequest,
    responses(
        (status = 200, description = "更新用户组成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "用户不存在"),
    ),
    tag = "users"
)]
#[axum::debug_handler]
async fn update_user_group(
    State(state): State<ServiceState>,
    Path(id): Path<i32>,
    user: AuthUser,
    Json(data): Json<UpdateUserGroupRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    // 检查是否为管理员
    require_admin(&user)?;

    let service = UserService::new(state.db);
    let user = service.update_user_group(id, data).await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    delete,
    path = "/api/users/{id}",
    params(
        ("id" = i32, Path, description = "用户ID")
    ),
    responses(
        (status = 200, description = "删除用户成功"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "用户不存在"),
    ),
    tag = "users"
)]
async fn delete_user(
    State(state): State<ServiceState>,
    Path(id): Path<i32>,
    user: AuthUser,
) -> Result<Json<ApiResponse<()>>> {
    // 检查是否为管理员
    require_admin(&user)?;

    let service = UserService::new(state.db);
    service.delete(id).await?;

    Ok(Json(ApiResponse::success_with_message((), "用户删除成功")))
}

#[utoipa::path(
    put,
    path = "/api/users/{id}/registration-status",
    params(
        ("id" = i32, Path, description = "用户ID")
    ),
    request_body = UpdateRegistrationStatusRequest,
    responses(
        (status = 200, description = "更新注册状态成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 404, description = "用户不存在"),
    ),
    tag = "users"
)]
async fn update_registration_status(
    State(state): State<ServiceState>,
    Path(id): Path<i32>,
    Json(data): Json<UpdateRegistrationStatusRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    let service = UserService::new(state.db);
    let user = service.update_registration_status(id, data).await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    put,
    path = "/api/users/{id}/season",
    params(
        ("id" = i32, Path, description = "用户ID")
    ),
    request_body = UpdateSeasonRequest,
    responses(
        (status = 200, description = "更新赛季成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 404, description = "用户不存在"),
    ),
    tag = "users"
)]
async fn update_season(
    State(state): State<ServiceState>,
    Path(id): Path<i32>,
    Json(data): Json<UpdateSeasonRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    let service = UserService::new(state.db);
    let user = service.update_season(id, data).await?;

    Ok(Json(ApiResponse::success(UserResponse::from(user))))
}

#[utoipa::path(
    put,
    path = "/api/users/{id}/approve",
    params(
        ("id" = i32, Path, description = "用户ID")
    ),
    request_body = UpdateApprovedRequest,
    responses(
        (status = 200, description = "更新审批状态成功", body = UserResponse),
        (status = 400, description = "请求参数错误"),
        (status = 401, description = "未授权"),
        (status = 403, description = "权限不足"),
        (status = 404, description = "用户不存在"),
    ),
    tag = "users"
)]
#[axum::debug_handler]
async fn update_approved(
    State(state): State<ServiceState>,
    Path(id): Path<i32>,
    user: AuthUser,
    Json(data): Json<UpdateApprovedRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    // 检查是否为管理员
    require_admin(&user)?;

    let service = UserService::new(state.db);
    let user = service.update_approved(id, data, true).await?;

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
