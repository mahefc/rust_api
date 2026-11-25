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
