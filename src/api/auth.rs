use axum::{Router, routing::get};

pub fn routes() -> Router {
    Router::new()
        .route("/osu", get(osu_auth))
        .route("/callback/osu", get(osu_callback))
        .route("/refresh", get(refresh_token))
        .route("/logout", get(logout))
}

async fn osu_auth() -> &'static str {
    "osu! OAuth 认证"
}

async fn osu_callback() -> &'static str {
    "osu! OAuth 回调"
}

async fn refresh_token() -> &'static str {
    "刷新Token"
}

async fn logout() -> &'static str {
    "登出"
}
