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
