use axum::http::StatusCode;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("validation error: {message}")]
    Validation { message: String },

    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("email already taken")]
    EmailTaken,

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("not found")]
    NotFound,

    #[error("conflict")]
    Conflict,

    #[error("database operation failed")]
    Database(#[source] sqlx::Error),

    #[error("internal application error")]
    Internal(#[from] anyhow::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::InvalidCredentials | Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::EmailTaken | Self::Conflict => StatusCode::CONFLICT,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Database(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "VALIDATION_ERROR",
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::EmailTaken => "EMAIL_TAKEN",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::Database(_) | Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    pub fn public_message(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "The request contains invalid data.",
            Self::InvalidCredentials => "Invalid email or password.",
            Self::EmailTaken => "An account with this email already exists.",
            Self::Unauthorized => "Authentication is required.",
            Self::Forbidden => "You do not have access to this resource.",
            Self::NotFound => "The requested resource was not found.",
            Self::Conflict => "The request conflicts with the current resource state.",
            Self::Database(_) | Self::Internal(_) => "An unexpected error occurred.",
        }
    }

    pub fn log_internal_error(&self) {
        match self {
            Self::Database(error) => {
                tracing::error!(
                    error = ?error,
                    error_code = self.code(),
                    "Database operation failed"
                );
            }
            Self::Internal(error) => {
                tracing::error!(
                    error = ?error,
                    error_code = self.code(),
                    "Internal application failure"
                )
            }
            _ => {}
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: ErrorDetails,
}

#[derive(Debug, Serialize)]
struct ErrorDetails {
    code: &'static str,
    message: String,
    status: u16,
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        self.log_internal_error();

        let status = self.status_code();

        let body = ErrorBody {
            error: ErrorDetails {
                code: self.code(),
                message: self.public_message().to_owned(),
                status: status.as_u16(),
            },
        };

        (status, axum::Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
