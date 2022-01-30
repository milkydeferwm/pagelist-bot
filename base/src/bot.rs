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
