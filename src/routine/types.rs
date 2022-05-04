#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct TaskConfig {
    pub timeout: u64,
    pub querylimit: i64,
}

impl TaskConfig {
    pub fn new() -> Self {
        TaskConfig {
            timeout: 0,
            querylimit: 0,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct SiteConfig {
    pub activate: bool,
    pub taskdir: String,
    pub resultheader: String,
    pub denyns: Vec<mediawiki::api::NamespaceID>,
    pub default: TaskConfig,
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct TaskInfo {
    pub activate: bool,
    pub description: String,
    pub expr: String,
    pub cron: String,
    pub eager: Option<bool>,
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
