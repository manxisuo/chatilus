use serde::Serialize;

pub type AppResult<T> = Result<T, AppError>;

/// Application error type. Serializes to a plain message string for Tauri commands.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("数据库错误: {0}")]
    Db(String),
    #[error("IO 错误: {0}")]
    Io(String),
    #[error("未找到: {0}")]
    NotFound(String),
    #[error("非法参数: {0}")]
    Invalid(String),
    #[error("路径不允许: {0}")]
    PathDenied(String),
    #[error("导入失败: {0}")]
    Import(String),
    #[error("数据库锁失败")]
    Lock,
}

impl AppError {
    pub fn msg(msg: impl Into<String>) -> Self {
        Self::Msg(msg.into())
    }
}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        Self::Msg(value)
    }
}

impl From<&str> for AppError {
    fn from(value: &str) -> Self {
        Self::Msg(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
