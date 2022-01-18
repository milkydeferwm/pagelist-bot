use std::fmt;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum APIAssertType {
    Anon,
    User,
    Bot,
}

impl fmt::Display for APIAssertType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Anon => f.write_str("anon"),
            Self::User => f.write_str("user"),
            Self::Bot => f.write_str("bot"),
        }
    }
}
