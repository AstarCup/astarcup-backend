pub mod auth;
pub mod user;

use axum::{Router, routing::get};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::service::ServiceState;

pub fn configure_routes() -> Router<ServiceState> {
    Router::new()
        // Swagger UI文档
        .merge(SwaggerUi::new("/api/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/health", get(|| async { "OK" }))
        .nest("/api/auth", auth::routes())
        .nest("/api/users", user::routes())
        .with_state(())
}

#[derive(OpenApi)]
#[openapi(
    paths(
        user::get_users,
        user::get_user,
        user::update_user,
        user::update_user_group,
        user::delete_user,
    ),
    components(
        schemas(
            crate::dto::user::UserResponse,
            crate::dto::user::UpdateUserRequest,
            crate::dto::user::UpdateUserGroupRequest,
            crate::dto::user::UserQueryParams,
            crate::dto::ApiResponse<crate::dto::user::UserResponse>,
            crate::dto::PaginatedResponse<crate::dto::user::UserResponse>,
            user::PaginatedUserResponse,
        )
    ),
    tags(
        (name = "auth", description = "认证相关API"),
        (name = "users", description = "用户管理API"),
    )
)]
pub struct ApiDoc;
