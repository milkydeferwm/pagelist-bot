#![allow(dead_code)]
use mediawiki::api::NamespaceID;

// Default namespace constants. Bundled with standard MediaWiki installations and could be seen as universal.
pub const NS_MAIN: NamespaceID = 0;
pub const NS_TALK: NamespaceID = 1;
pub const NS_USER: NamespaceID = 2;
pub const NS_USER_TALK: NamespaceID = 3;
pub const NS_PROJECT: NamespaceID = 4;
pub const NS_PROJECT_TALK: NamespaceID = 5;
pub const NS_FILE: NamespaceID = 6;
pub const NS_FILE_TALK: NamespaceID = 7;
pub const NS_MEDIAWIKI: NamespaceID = 8;
pub const NS_MEDIAWIKI_TALK: NamespaceID = 9;
pub const NS_TEMPLATE: NamespaceID = 10;
pub const NS_TEMPLATE_TALK: NamespaceID = 11;
pub const NS_HELP: NamespaceID = 12;
pub const NS_HELP_TALK: NamespaceID = 13;
pub const NS_CATEGORY: NamespaceID = 14;
pub const NS_CATEGORY_TALK: NamespaceID = 15;
pub const NS_SPECIAL: NamespaceID = -1;
pub const NS_MEDIA: NamespaceID = -2;