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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code_permission_denied() {
        assert_eq!(LokiError::PermissionDenied.exit_code(), 2);
    }

    #[test]
    fn test_exit_code_timeout() {
        assert_eq!(LokiError::Timeout(5000).exit_code(), 3);
    }

    #[test]
    fn test_exit_code_defaults_to_1() {
        assert_eq!(LokiError::ElementNotFound("x".into()).exit_code(), 1);
        assert_eq!(LokiError::WindowNotFound("x".into()).exit_code(), 1);
        assert_eq!(LokiError::AppNotFound("x".into()).exit_code(), 1);
        assert_eq!(LokiError::LaunchFailed("x".into()).exit_code(), 1);
        assert_eq!(LokiError::InputError("x".into()).exit_code(), 1);
        assert_eq!(LokiError::ScreenshotFailed("x".into()).exit_code(), 1);
        assert_eq!(LokiError::Platform("x".into()).exit_code(), 1);
    }

    #[test]
    fn test_display_messages() {
        assert_eq!(
            LokiError::ElementNotFound("btn".into()).to_string(),
            "element not found: btn"
        );
        assert_eq!(
            LokiError::WindowNotFound("main".into()).to_string(),
            "window not found: main"
        );
        assert_eq!(
            LokiError::Timeout(3000).to_string(),
            "timed out after 3000ms"
        );
        assert_eq!(
            LokiError::PermissionDenied.to_string(),
            "permission denied — grant Accessibility access in System Settings"
        );
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let loki_err: LokiError = io_err.into();
        assert_eq!(loki_err.exit_code(), 1);
        assert!(loki_err.to_string().contains("file missing"));
    }
}
