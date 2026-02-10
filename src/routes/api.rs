use axum::{
    Router,
    extract::State,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;

use crate::{
    config::Settings,
    database::Database,
    handlers::{
        auth::{auth_callback, get_auth_url, logout, verify_token},
        health::health_check,
        match_room::{
            create_match_room, delete_match_room, get_match_room, get_match_rooms,
            get_match_rooms_count, update_match_room,
        },
        message::{
            delete_message, get_message, get_messages, get_messages_count,
            get_unread_messages_count, send_message, update_message,
        },
        registration::{
            approve_registration, delete_registration, get_approved_players, get_registrations,
            get_registrations_count,
        },
        team::{get_team, get_teams, get_teams_count},
        tournament_config::{
            create_tournament_config, delete_tournament_config, get_current_tournament_config,
            get_tournament_config, get_tournament_configs, get_tournament_configs_count,
            update_tournament_config,
        },
        user::{get_user, get_user_by_osuid, get_users, get_users_count},
    },
    services::auth::AuthService,
};

pub fn create_router(database: Database, settings: Settings) -> Router {
    let auth_service = AuthService::new(settings.clone());

    Router::new()
        // Health check
        .route("/health", get(health_check))
        // Auth routes
        .route("/api/auth/url", get(get_auth_url))
        .route("/api/auth/callback/osu", get(auth_callback))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/verify", get(verify_token_handler))
        // User routes
        .route("/api/users", get(get_users))
        .route("/api/users/count", get(get_users_count))
        .route("/api/users/:id", get(get_user))
        .route("/api/users/osuid/:osuid", get(get_user_by_osuid))
        // Registration routes
        .route("/api/registrations", get(get_registrations))
        .route("/api/registrations/count", get(get_registrations_count))
        .route("/api/registrations/approved", get(get_approved_players))
        .route(
            "/api/admin/registrations/:id/approve",
            post(approve_registration),
        )
        .route("/api/admin/registrations/:id", delete(delete_registration))
        // Match room routes
        .route("/api/match-rooms", get(get_match_rooms))
        .route("/api/match-rooms/count", get(get_match_rooms_count))
        .route("/api/match-rooms/:id", get(get_match_room))
        .route("/api/match-rooms", post(create_match_room))
        .route("/api/match-rooms/:id", put(update_match_room))
        .route("/api/match-rooms/:id", delete(delete_match_room))
        // Message routes
        .route("/api/messages", get(get_messages))
        .route("/api/messages/count", get(get_messages_count))
        .route("/api/messages/:id", get(get_message))
        .route("/api/messages", post(send_message))
        .route("/api/messages/:id", put(update_message))
        .route("/api/messages/:id", delete(delete_message))
        .route(
            "/api/messages/unread/:receiver_id",
            get(get_unread_messages_count),
        )
        // Tournament config routes
        .route("/api/tournament-configs", get(get_tournament_configs))
        .route(
            "/api/tournament-configs/count",
            get(get_tournament_configs_count),
        )
        .route(
            "/api/tournament-configs/current",
            get(get_current_tournament_config),
        )
        .route("/api/tournament-configs/:id", get(get_tournament_config))
        .route("/api/tournament-configs", post(create_tournament_config))
        .route("/api/tournament-configs/:id", put(update_tournament_config))
        .route(
            "/api/tournament-configs/:id",
            delete(delete_tournament_config),
        )
        // Team routes
        .route("/api/teams", get(get_teams))
        .route("/api/teams/count", get(get_teams_count))
        .route("/api/teams/:id", get(get_team))
        .with_state(auth_service)
        .with_state(database)
}

// Handler for token verification with header extraction
async fn verify_token_handler(
    TypedHeader(auth_header): TypedHeader<Authorization<Bearer>>,
    State(auth_service): State<AuthService>,
) -> impl IntoResponse {
    verify_token(State(auth_service), auth_header.token().to_string()).await
}
