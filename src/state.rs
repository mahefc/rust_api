use std::env::var;
use std::time::Instant;

use axum::{extract::Request, middleware::Next, response::Response};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, fmt::time::ChronoLocal};

use crate::services::user_service;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

pub async fn connect() -> PgPool {
    dotenvy::dotenv().ok();
    let database_url = var("DATABASE_URL").expect("DATABASE_URL is not set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();

    user_service::create_table(&pool).await.unwrap();
    info!("database ready");

    pool
}

pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx::postgres::notice=warn"));

    let time = ChronoLocal::new("%Y-%m-%d %H:%M:%S".to_string());

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_timer(time)
        .init();
}

// Logs one line per request: method, uri, status, latency
pub async fn log_requests(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    let response = next.run(req).await;

    info!(
        "{} {} {} {}ms",
        method,
        uri,
        response.status().as_u16(),
        start.elapsed().as_millis()
    );
    response
}
