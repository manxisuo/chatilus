use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataSource {
    ChatGpt,
    Cursor,
    Codex,
    Claude,
    Gemini,
    DeepSeek,
    Copilot,
}

impl DataSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChatGpt => "chatgpt",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::DeepSeek => "deepseek",
            Self::Copilot => "copilot",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "cursor" => Self::Cursor,
            "codex" => Self::Codex,
            "claude" => Self::Claude,
            "gemini" => Self::Gemini,
            "deepseek" => Self::DeepSeek,
            "copilot" => Self::Copilot,
            _ => Self::ChatGpt,
        }
    }
}

impl Default for DataSource {
    fn default() -> Self {
        Self::ChatGpt
    }
}
