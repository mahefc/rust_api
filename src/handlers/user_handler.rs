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
