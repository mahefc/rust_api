# RustAPI: Step-by-Step Guide

This guide shows how the project was built, step by step: create the project, run it, add Axum, connect Postgres, add routes, then handlers and services.

Stack: Rust, Axum (web framework), SQLx (Postgres), Tokio (async runtime).

---

## 1. Create the project

```bash
cargo new RustAPI
cd RustAPI
```

Run it to check Rust works:

```bash
cargo run
```

You should see `Hello, world!`.

### Windows note (linker)

On Windows, the default toolchain needs Visual Studio Build Tools. If you don't have them, use the GNU toolchain (needs MinGW):

```bash
rustup default stable-x86_64-pc-windows-gnu
```

---

## 2. Add Axum and Tokio

Axum runs on Tokio, so add both:

```bash
cargo add axum
cargo add tokio --features full
```

Your `Cargo.toml` `[dependencies]` should now have `axum` and `tokio`.

---

## 3. First route

`src/main.rs`:

```rust
use axum::{Router, routing::get};
use tokio::net::TcpListener;

fn routes() -> Router {
    Router::new().route("/test", get(test_handler))
}

async fn test_handler() -> &'static str {
    "App is running!"
}

#[tokio::main]
async fn main() {
    let app = routes();
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
```

Run with `cargo run`, then open `http://127.0.0.1:3000/test`.

Key ideas:
- `Router` maps URLs to handlers.
- A handler is an `async fn` that returns something Axum can turn into a response.
- `#[tokio::main]` starts the async runtime.

---

## 4. Add Postgres (SQLx)

```bash
cargo add sqlx --features runtime-tokio,postgres
cargo add dotenvy
```

### Environment file

Create `.env` in the project root:

```
DATABASE_URL=postgres://USER:PASSWORD@localhost:5432/DBNAME
```

Optional logging variables (see section 8):

```
RUST_LOG=info
LOG_FORMAT=json
```

Add it to `.gitignore` so the password is never committed:

```
/target
.env
```

### Connection code

`src/state.rs` holds the shared state and the connection function:

```rust
use sqlx::{PgPool, postgres::PgPoolOptions};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

pub async fn connect() -> PgPool {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is not set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();

    // Create the users table on startup if it does not exist yet
    user_service::create_table(&pool).await.unwrap();
    info!("database ready");

    pool
}
```

Add these imports at the top of `state.rs`: `use tracing::info;` and `use crate::services::user_service;`.

- `dotenvy` reads `.env`.
- `AppState` is cloned into every handler, so it should only hold cheap-to-clone things like a pool.

Register the module and connect in `main.rs`:

```rust
mod state;
use state::AppState;

// inside main:
let pool = state::connect().await;
let app = routes(AppState { db: pool });
```

Update `routes` to accept the state:

```rust
fn routes(state: AppState) -> Router {
    Router::new()
        .route("/test", get(test_handler))
        .with_state(state)
}
```

---

## 5. Models

A model is a struct that matches a table row. Put models in `src/models/`.

```bash
mkdir src/models
```

`src/models/mod.rs`:

```rust
pub mod user;
```

`src/models/user.rs`:

```rust
use serde::{Deserialize, Serialize};

// Columns every query returns, in the same order as the User struct
pub const COLUMNS: &str = "id, name, email";

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    // Set by the database. Any id sent by the client is ignored.
    #[serde(skip_deserializing)]
    pub id: i32,
    pub name: String,
    pub email: String,
}
```

Keeping the column list next to the struct means the two can't drift apart: if you add a field, update `COLUMNS` in the same file.

Add serde:

```bash
cargo add serde --features derive
```

- `Serialize` turns the struct into JSON (responses).
- `Deserialize` reads JSON into the struct (requests).
- `sqlx::FromRow` lets SQLx fill the struct from a database row.

Add `mod models;` to `main.rs`.

---

## 6. Services (database layer)

A service only talks to the database. It has no HTTP code. Put services in `src/services/`.

```bash
mkdir src/services
```

`src/services/mod.rs`:

```rust
pub mod user_service;
```

`src/services/user_service.rs`:

```rust
use sqlx::PgPool;

use crate::models::user::{COLUMNS, User};

// Fills {COLUMNS} in a SQL template. Only our own constant is inserted, never user input.
fn user_sql(template: &str) -> sqlx::AssertSqlSafe<String> {
    sqlx::AssertSqlSafe(template.replace("{COLUMNS}", COLUMNS))
}

// Creates the users table if it does not exist yet
pub async fn create_table(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert(pool: &PgPool, input: &User) -> Result<User, sqlx::Error> {
    sqlx::query_as(user_sql(
        "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING {COLUMNS}",
    ))
    .bind(&input.name)
    .bind(&input.email)
    .fetch_one(pool)
    .await
}

pub async fn find_all(pool: &PgPool) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as(user_sql("SELECT {COLUMNS} FROM users ORDER BY id"))
        .fetch_all(pool)
        .await
}

pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as(user_sql("SELECT {COLUMNS} FROM users WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn update(pool: &PgPool, id: i32, input: &User) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as(user_sql(
        "UPDATE users SET name = $1, email = $2 WHERE id = $3 RETURNING {COLUMNS}",
    ))
    .bind(&input.name)
    .bind(&input.email)
    .bind(id)
    .fetch_optional(pool)
    .await
}

// Returns true if a row was deleted
pub async fn delete(pool: &PgPool, id: i32) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
```

Rules:
- Use `$1, $2...` placeholders and `.bind(...)`. Never put user input into the SQL string.
- Column names come from `COLUMNS` through `user_sql`, which wraps the SQL in `sqlx::AssertSqlSafe` (SQLx 0.9 requires this for runtime-built SQL). Only use it with constants, never user input.
- Return `Option` when a row may not exist.

The table is created at startup by `state::connect()` (see section 4), so `main.rs` doesn't need to call it.

---

## 7. Handlers (HTTP layer)

A handler reads the request, calls a service, and builds the response. Put handlers in `src/handlers/`.

```bash
mkdir src/handlers
```

`src/handlers/mod.rs`:

```rust
pub mod user_handler;
```

`src/handlers/user_handler.rs`:

```rust
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use tracing::{error, info, warn};

use crate::models::user::User;
use crate::services::user_service;
use crate::state::AppState;

type ApiError = (StatusCode, String);

// Logs the real error on the server, but sends only a generic message to the client
fn db_error(e: sqlx::Error) -> ApiError {
    error!(error = %e, "database error");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".to_string())
}

fn not_found(id: i32) -> ApiError {
    warn!(user_id = id, "user not found");
    (StatusCode::NOT_FOUND, "user not found".to_string())
}

// POST /users
pub async fn create_user(
    State(state): State<AppState>,
    Json(input): Json<User>,
) -> Result<Json<User>, ApiError> {
    let user = user_service::insert(&state.db, &input).await.map_err(db_error)?;
    info!(user_id = user.id, "user created");
    Ok(Json(user))
}

// GET /users
pub async fn list_users(State(state): State<AppState>) -> Result<Json<Vec<User>>, ApiError> {
    let users = user_service::find_all(&state.db).await.map_err(db_error)?;
    Ok(Json(users))
}

// GET /users/{id}
pub async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<User>, ApiError> {
    let user = user_service::find_by_id(&state.db, id).await.map_err(db_error)?;
    user.map(Json).ok_or_else(|| not_found(id))
}

// PUT /users/{id}
pub async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(input): Json<User>,
) -> Result<Json<User>, ApiError> {
    let user = user_service::update(&state.db, id, &input).await.map_err(db_error)?;
    match user {
        Some(u) => {
            info!(user_id = id, "user updated");
            Ok(Json(u))
        }
        None => Err(not_found(id)),
    }
}

// DELETE /users/{id}
pub async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    let deleted = user_service::delete(&state.db, id).await.map_err(db_error)?;
    if deleted {
        info!(user_id = id, "user deleted");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found(id))
    }
}
```

Handler rules:
- Extractors (`State`, `Path`, `Json`) pull data from the request.
- Return `Result<Json<T>, ApiError>` so errors become HTTP responses.
- Keep business logic in the handler minimal; database work belongs in the service.
- Never send raw database errors to the client. Log them with `error!` and return a generic message.

---

## 8. Logging

### Why `tracing` instead of the built-in logger

Rust's standard library has no logger. `println!` works, but it has no levels, no filtering, no timestamps, and no way to switch it off per module. For a production API that isn't enough.

- **`log` crate:** adds levels (`info!`, `error!`) and works with any output library. It only sends plain messages.
- **`tracing`:** does everything `log` does, and also attaches fields (`user_id = 5`) and spans (the request context). Axum and SQLx use it, so their logs appear in the same output.

For a small app, `log` with `env_logger` is also fine. `tracing` is the standard choice for Axum and tokio apps, so this guide uses it.

Logging uses two crates:
- `tracing`: the logging macros (`info!`, `warn!`, `error!`).
- `tracing-subscriber`: prints the logs, as plain text or JSON, with local time.

```bash
cargo add tracing
cargo add tracing-subscriber --features env-filter,json,chrono
```

The logging functions live in `src/state.rs`, next to the connection code. Add these imports to the top of `state.rs`: `std::time::Instant`, `axum::{extract::Request, middleware::Next, response::Response}`, `tracing::info`, and `tracing_subscriber::{EnvFilter, fmt, fmt::time::ChronoLocal}`.

```rust
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
```

Call `state::init_logging()` first in `main`. The `log_requests` middleware is attached in section 9.

When to use each level:

| Macro | Use for | Example |
|---|---|---|
| `info!` | normal events | user created, server started |
| `warn!` | expected problems | user not found |
| `error!` | unexpected failures | database error |
| `debug!` | details for development | shown with `RUST_LOG=debug` |

Fields such as `user_id = id` become separate JSON fields, so production log tools can filter by them.

Example output in text mode (default):

```
2026-11-11 18:19:22  INFO database ready
2026-11-11 18:19:33  INFO user created user_id=12
2026-11-11 18:19:33  INFO POST /users 200 8ms
2026-11-11 18:19:33  INFO GET /users/1 200 5ms
2026-11-11 18:19:33  WARN user not found user_id=99999
2026-11-11 18:19:33  INFO GET /users/99999 404 2ms
```

Each request is one line: date, time, level, method, URI, status, latency. With `LOG_FORMAT=json` the same events print as one JSON object per line, for production log tools.

---

## 9. Wire up the routes

`src/main.rs` (final version):

```rust
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
        // Logs every request on one line (method, uri, status, latency)
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
```

Path parameters use the `{id}` syntax in Axum 0.8.

---

## 10. Run and test

```bash
cargo run
```

Set `DATABASE_URL` in `.env` first.

| Method | URL | Body | Result |
|---|---|---|---|
| GET | `/users` | - | list users |
| POST | `/users` | `{"name":"A","email":"a@x.com"}` | 200, new user |
| GET | `/users/1` | - | 200 or 404 |
| PUT | `/users/1` | `{"name":"B","email":"b@x.com"}` | 200 or 404 |
| DELETE | `/users/1` | - | 204 or 404 |

Test with curl:

```bash
curl -X POST http://127.0.0.1:3000/users -H "Content-Type: application/json" -d "{\"name\":\"A\",\"email\":\"a@x.com\"}"
curl http://127.0.0.1:3000/users
```

---

## Project layout

```
RustAPI/
├── .env                      # DATABASE_URL, LOG_FORMAT (not committed)
├── .gitignore
├── Cargo.toml
├── GUIDE.md
└── src/
    ├── main.rs               # routes + startup
    ├── state.rs              # AppState, DB connection, logging
    ├── models/
    │   ├── mod.rs
    │   └── user.rs           # User struct
    ├── services/
    │   ├── mod.rs
    │   └── user_service.rs   # SQL queries
    └── handlers/
        ├── mod.rs
        └── user_handler.rs   # HTTP handlers
```

Request flow:

```
HTTP request → Router + log middleware (main.rs) → Handler (handlers/) → Service (services/) → Postgres
                                       ↓
HTTP response ← Json / StatusCode ←────┘
```
