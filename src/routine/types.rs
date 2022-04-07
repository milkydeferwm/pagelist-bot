#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct TaskInfo {
    pub activate: bool,
    pub description: String,
    pub expr: String,
    pub interval: u64,
    pub timeout: Option<u64>,
    pub querylimit: Option<i64>,
    pub output: Vec<OutputFormat>,
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct OutputFormatSuccess {
    pub before: String,
    pub item: String,
    pub between: String,
    pub after: String,
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct OutputFormat {
    pub target: String,
    pub failure: String,
    pub empty: String,
    pub success: OutputFormatSuccess,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TaskStatus {
    Standby,
    Running,
    Dead,
}

#[derive(Debug)]
pub enum EditPageError {
    BadTitle,
    RedirectOrMissing,
    MediaWiki(mediawiki::media_wiki_error::MediaWikiError),
    EditError(String, String),
}

impl std::error::Error for EditPageError {}
unsafe impl Send for EditPageError {}

impl std::fmt::Display for EditPageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadTitle => f.write_str("bad title"),
            Self::RedirectOrMissing => f.write_str("target page is missing or is a redirect"),
            Self::MediaWiki(e) => e.fmt(f),
            Self::EditError(code, info) => f.write_fmt(format_args!("MediaWiki API returns error code: \"{}\", more info: \"{}\"", code, info)),
        }
    }
}

impl From<mediawiki::media_wiki_error::MediaWikiError> for EditPageError {
    fn from(e: mediawiki::media_wiki_error::MediaWikiError) -> Self {
        Self::MediaWiki(e)
    }
}

impl From<(String, String)> for EditPageError {
    fn from(e: (String, String)) -> Self {
        Self::EditError(e.0, e.1)
    }
}
