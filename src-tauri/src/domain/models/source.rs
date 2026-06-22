use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataSource {
    ChatGpt,
    Cursor,
    Claude,
    Gemini,
}

impl DataSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChatGpt => "chatgpt",
            Self::Cursor => "cursor",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "cursor" => Self::Cursor,
            "claude" => Self::Claude,
            "gemini" => Self::Gemini,
            _ => Self::ChatGpt,
        }
    }
}

impl Default for DataSource {
    fn default() -> Self {
        Self::ChatGpt
    }
}
