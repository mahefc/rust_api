mod handlers;
mod models;
mod services;
mod state;

use axum::{Router, middleware, routing::get};
use handlers::user_handler;
use state::AppState;
use tokio::net::TcpListener;
use tracing::info;

fn routes(state: AppState) -> Router {
    Router::new()
        .route("/test", get(test_handler))
        .route(
            "/users",
            get(user_handler::list_users).post(user_handler::create_user),
        )
        .route(
            "/users/{id}",
            get(user_handler::get_user)
                .put(user_handler::update_user)
                .delete(user_handler::delete_user),
        )
        .with_state(state)
        .layer(middleware::from_fn(state::log_requests))
}

async fn test_handler() -> &'static str {
    "App is running!"
}

#[tokio::main]
async fn main() {
    state::init_logging();

    let pool = state::connect().await;

    let app = routes(AppState { db: pool });
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    info!("listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
