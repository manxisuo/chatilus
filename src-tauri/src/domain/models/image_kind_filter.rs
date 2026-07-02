#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageKindFilter {
    #[default]
    All,
    Generated,
    Upload,
}

impl ImageKindFilter {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "all" => Some(Self::All),
            "generated" => Some(Self::Generated),
            "upload" | "uploads" => Some(Self::Upload),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Generated => "generated",
            Self::Upload => "upload",
        }
    }
}
