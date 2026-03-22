use thiserror::Error;

/// Core error type for all Loki operations.
#[derive(Debug, Error)]
pub enum LokiError {
    #[error("element not found: {0}")]
    ElementNotFound(String),

    #[error("window not found: {0}")]
    WindowNotFound(String),

    #[error("app not found: {0}")]
    AppNotFound(String),

    #[error("launch failed: {0}")]
    LaunchFailed(String),

    #[error("permission denied — grant Accessibility access in System Settings")]
    PermissionDenied,

    #[error("timed out after {0}ms")]
    Timeout(u64),

    #[error("input error: {0}")]
    InputError(String),

    #[error("screenshot failed: {0}")]
    ScreenshotFailed(String),

    #[error("platform error: {0}")]
    Platform(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl LokiError {
    /// Map error variants to CLI exit codes.
    pub fn exit_code(&self) -> i32 {
        match self {
            LokiError::PermissionDenied => 2,
            LokiError::Timeout(_) => 3,
            _ => 1,
        }
    }
}

pub type LokiResult<T> = Result<T, LokiError>;
