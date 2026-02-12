pub mod auth;

use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

pub fn configure_middleware() -> impl tower::Layer<axum::Router> + Clone {
    ServiceBuilder::new().layer(TraceLayer::new_for_http())
}
