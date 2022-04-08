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
    pub botflag: bool,
    pub config: String,
}
