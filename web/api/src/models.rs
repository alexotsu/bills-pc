use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: Option<String>,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub oauth_provider: Option<String>,
    #[serde(skip_serializing)]
    pub oauth_subject: Option<String>,
    pub training_data_opt_in: bool,
    pub created_at: DateTime<Utc>,
}
