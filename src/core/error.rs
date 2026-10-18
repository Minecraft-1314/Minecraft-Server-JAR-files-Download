use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    Validation(String),
    General(String),
}

impl AppError {
    pub fn validation(code: impl Into<String>) -> Self {
        AppError::Validation(code.into())
    }

    pub fn general(message: impl Into<String>) -> Self {
        AppError::General(message.into())
    }

    pub fn message(&self) -> &str {
        match self {
            AppError::Validation(code) => code,
            AppError::General(message) => message,
        }
    }

    pub fn is_validation(&self) -> bool {
        matches!(self, AppError::Validation(_))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::General(error.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        AppError::General(error.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
