#[derive(Debug)]
pub enum PLBotParserError {
    Parse,
    Semantic(String),
}

impl std::error::Error for PLBotParserError {}

impl std::fmt::Display for PLBotParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse => f.write_str("parse fails"),
            Self::Semantic(s) => f.write_fmt(format_args!("semantic error: {}", s)),
        }
    }
}
