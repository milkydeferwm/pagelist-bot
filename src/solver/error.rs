use std::error::Error;
use std::fmt;

use crate::apiservice::APIServiceError;

#[derive(Debug)]
pub enum SolveError {
    MediaWiki(mediawiki::media_wiki_error::MediaWikiError),
    APIService(APIServiceError),
    QueryForMultiplePages,
    UnknownIntermediateValue,
    NotCategory,
}

impl Error for SolveError {}
unsafe impl Send for SolveError {}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MediaWiki(e) => e.fmt(f),
            Self::QueryForMultiplePages => f.write_str("cannot query for multiple pages"),
            Self::APIService(e) => f.write_fmt(format_args!("API Service fails with error: \"{}\"", e)),
            Self::UnknownIntermediateValue => f.write_str("cannot access an intermediate value before it is initialized"),
            Self::NotCategory => f.write_str("cannot query for members of something not a category"),
        }
    }
}

impl From<mediawiki::media_wiki_error::MediaWikiError> for SolveError {
    fn from(e: mediawiki::media_wiki_error::MediaWikiError) -> Self {
        Self::MediaWiki(e)
    }
}

impl From<APIServiceError> for SolveError {
    fn from(e: APIServiceError) -> Self {
        Self::APIService(e)
    }
}
