use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrabotError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Skill requirement not met: {0}")]
    SkillRequirementNotMet(String),

    #[error("Promotion failed: {0}")]
    PromotionFailed(String),

    #[error("SQL error: {0}")]
    SqlError(#[from] sqlx::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, CrabotError>;
