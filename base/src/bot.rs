#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum APIAssertType {
    Anon,
    User,
    Bot,
}

impl ToString for APIAssertType {
    fn to_string(&self) -> String {
        match *self {
            Self::Anon => String::from("anon"),
            Self::User => String::from("user"),
            Self::Bot => String::from("bot"),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct LoginCredential {
    pub username: String,
    pub password: String,
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct SiteProfile {
    pub api: String,
    pub db: Option<String>,
    pub login: String,
    pub assert: Option<APIAssertType>,
    pub config: String,
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct SiteConfig {
    pub activate: bool,
    pub taskdir: String,
    pub interval: u64,
    pub resultheader: String,
    pub default: TaskConfig,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Deserialize)]
pub struct TaskConfig {
    pub timeout: u64,
    pub querylimit: u64,
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct TaskInfo {
    pub activate: bool,
    pub description: String,
    pub expr: String,
    pub interval: u64,
    pub timeout: Option<u64>,
    pub querylimit: Option<u64>,
    pub output: Vec<OutputFormat>,
}

#[derive(PartialEq, Eq, Clone, Debug, serde::Deserialize)]
pub struct OutputFormat {
    pub target: String,
    pub before: String,
    pub item: String,
    pub between: String,
    pub after: String,
}
