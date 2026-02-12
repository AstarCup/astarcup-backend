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
            CreateUserRequest, UpdateUserGroupRequest, UpdateUserRequest, UserQueryParams,
            UserResponse,
        },
    },
    error::Result,
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
async fn update_user(
    State(state): State<ServiceState>,
    Path(id): Path<i32>,
    Json(data): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
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
async fn update_user_group(
    State(state): State<ServiceState>,
    Path(id): Path<i32>,
    Json(data): Json<UpdateUserGroupRequest>,
) -> Result<Json<ApiResponse<UserResponse>>> {
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
) -> Result<Json<ApiResponse<()>>> {
    let service = UserService::new(state.db);
    service.delete(id).await?;

    Ok(Json(ApiResponse::success_with_message((), "用户删除成功")))
}

#[derive(ToSchema)]
pub struct PaginatedUserResponse {
    items: Vec<UserResponse>,
    total: u64,
    page: u64,
    page_size: u64,
    total_pages: u64,
}
