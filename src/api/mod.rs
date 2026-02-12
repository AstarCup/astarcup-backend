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
}

#[derive(OpenApi)]
#[openapi(
    paths(
        auth::osu_auth,
        auth::osu_callback,
        auth::refresh_token,
        auth::logout,
        user::get_users,
        user::get_user,
        user::update_user,
        user::update_user_group,
        user::delete_user,
        user::update_registration_status,
        user::update_season,
        user::update_approved,
    ),
    components(
        schemas(
            crate::dto::user::UserResponse,
            crate::dto::user::UpdateUserRequest,
            crate::dto::user::UpdateUserGroupRequest,
            crate::dto::user::UpdateRegistrationStatusRequest,
            crate::dto::user::UpdateSeasonRequest,
            crate::dto::user::UpdateApprovedRequest,
            crate::dto::user::UserQueryParams,
            crate::dto::ApiResponseSchema<crate::dto::user::UserResponse>,
            crate::dto::ApiResponseSchema<crate::api::auth::AuthResponse>,
            crate::dto::PaginatedResponseSchema<crate::dto::user::UserResponse>,
            user::PaginatedUserResponse,
        )
    ),
    tags(
        (name = "auth", description = "认证相关API"),
        (name = "users", description = "用户管理API"),
    )
)]
pub struct ApiDoc;
