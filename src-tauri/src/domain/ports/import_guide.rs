use serde::{Deserialize, Serialize};

/// 某一数据源支持的一种导入方式（目录 / 文件等）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMethodGuide {
    pub id: String,
    pub label: String,
    /// `directory` 或 `file`
    pub kind: String,
    pub dialog_title: String,
    #[serde(default)]
    pub extensions: Vec<String>,
    pub hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example_path: Option<String>,
    /// 运行时探测到的本机默认路径（存在且可导入时填充）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_default_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_default_label: Option<String>,
}

/// 某一 Importer 的完整导入指引（供前端向导展示）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportGuide {
    pub importer_id: String,
    pub source: String,
    pub display_name: String,
    pub description: String,
    /// `stable` 或 `experimental`
    pub support_status: String,
    /// 卡片上展示的简要支持说明，如「ZIP / 解压目录」。
    pub support_summary: String,
    /// 识别依据：告诉用户 ChatLens 如何判断路径是否正确。
    pub recognition_hint: String,
    pub methods: Vec<ImportMethodGuide>,
}

/// 按当前操作系统返回展示用路径（Windows / macOS / Linux）。
pub fn platform_display_path(windows: &str, macos: &str, linux: &str) -> String {
    if cfg!(target_os = "windows") {
        windows.to_string()
    } else if cfg!(target_os = "macos") {
        macos.to_string()
    } else {
        linux.to_string()
    }
}
