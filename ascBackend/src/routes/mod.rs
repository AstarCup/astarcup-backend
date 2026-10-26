pub mod auth_routes;
pub mod beatmaps_routes;
pub mod bind_routes;
pub mod bot_routes;
pub mod content_routes;
pub mod maps_routes;
pub mod matchups_routes;
pub mod messages_routes;
pub mod news_routes;
pub mod osu_mp_routes;
pub mod replays_routes;
pub mod rooms_routes;
pub mod rules_routes;
pub mod schedules_routes;
pub mod settings_routes;
pub mod staff_routes;
pub mod users_routes;

use axum::Router;

use crate::state::AppState;

/// 汇总全部路由
pub fn build_router() -> Router<AppState> {
    Router::new()
        .merge(auth_routes::router())
        .merge(bind_routes::router())
        .merge(users_routes::router())
        .merge(rooms_routes::router())
        .merge(schedules_routes::router())
        .merge(matchups_routes::router())
        .merge(staff_routes::router())
        .merge(messages_routes::router())
        .merge(maps_routes::router())
        .merge(beatmaps_routes::router())
        .merge(replays_routes::router())
        .merge(osu_mp_routes::router())
        .merge(settings_routes::router())
        .merge(news_routes::router())
        .merge(rules_routes::router())
        .merge(content_routes::router())
        .merge(bot_routes::router())
}
