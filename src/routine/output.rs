use std::collections::HashMap;
use mediawiki::title::Title;
use mediawiki::page::Page;
use mediawiki::api::Api;
use plbot_base::bot::APIAssertType;
use md5::{Md5, Digest};

use super::types::EditPageError;

pub fn generate_text(list: &[Title], api: &Api, before_list_text: &str, list_item_text: &str, between_item_text: &str, after_list_text: &str) -> String {
    let list_size = list.len();
    let mut output: String = String::new();
    output.push_str(&substitute_str_template(before_list_text, list_size));
    let item_str: String = list.iter().enumerate().map(|(idx, t)| substitute_str_template_with_title(list_item_text, t, idx + 1, list_size, api)).collect::<Vec<String>>().join(&substitute_str_template(between_item_text, list_size));
    output.push_str(&item_str);
    output.push_str(&substitute_str_template(after_list_text, list_size));
    output
}

fn substitute_str_template(template: &str, total_num: usize) -> String {
    let mut output: String = String::new();
    let mut escape: bool = false;
    for char in template.chars() {
        if escape {
            // only accept $+ (total size), $$ ($)
            match char {
                '$' => { output.push('$'); },
                '+' => { output.push_str(&total_num.to_string()) },
                _ => { output.push('$'); output.push(char); },
            }
            escape = false;
        } else if char == '$' {
            escape = true;
        } else {
            output.push(char);
        }
    }
    output
}

fn substitute_str_template_with_title(template: &str, t: &Title, current_num: usize, total_num: usize, api: &Api) -> String {
    let mut output: String = String::new();
    let mut escape: bool = false;
    for char in template.chars() {
        if escape {
            // only accept $0 (full name), $1 (namespace), $2 (name), $@ (current index), $+ (total size), $$ ($)
            match char {
                '$' => { output.push('$'); },
                '0' => { output.push_str(&t.full_pretty(api).unwrap_or_else(|| "".to_string())); },
                '1' => { output.push_str(t.namespace_name(api).unwrap_or("")); },
                '2' => { output.push_str(t.pretty()); },
                '@' => { output.push_str(&current_num.to_string()) },
                '+' => { output.push_str(&total_num.to_string()) },
                _ => { output.push('$'); output.push(char); },
            }
            escape = false;
        } else if char == '$' {
            escape = true;
        } else {
            output.push(char);
        }
    }
    output
}

/// Replaces the contents of this `Page` with the given text, using the given
/// edit summary.
/// 
/// This version differs from `Page::edit_text` in `formatversion` and `assert`.
/// Also it uses md5.
/// 
/// 
pub async fn write_page(page: &Page, api: &mut Api, text: impl Into<String>, summary: impl Into<String>, assert: Option<APIAssertType>, bot: bool) -> Result<(), EditPageError> {
    let title = page.title().full_pretty(api).ok_or(EditPageError::BadTitle)?;
    // if the target page is a redirect or does not exist, stop
    let mut params = api.params_into(&[
        ("utf8", "1"),
        ("action", "query"),
        ("prop", "info"),
        ("titles", &title),
    ]);
    if let Some(a) = assert {
        params.insert("assert".to_string(), a.to_string());
    };
    let res = api.get_query_api_json_all(&params).await?;
    if let Some(res) = res["query"]["pages"].as_object() {
        for (_, v) in res.iter() {
            if v.get("missing").is_some() || v.get("redirect").is_some() {
                return Err(EditPageError::RedirectOrMissing);
            }
        }
    } else {
        return Err(EditPageError::EditError(res["error"]["code"].as_str().unwrap_or("<unknown>").to_string(), res["error"]["info"].as_str().unwrap_or("<unknown>").to_string()));
    }
    // else, continue
    let text_string = text.into();
    let mut hasher = Md5::new();
    hasher.update(&text_string);
    let result = hasher.finalize();
    let md5 = hex::encode(result);
    let mut params: HashMap<String, String> = [
        ("action", "edit"),
        ("title", &title),
        ("text", &text_string),
        ("summary", &summary.into()),
        ("utf8", "1"),
        ("md5", &md5),
        ("nocreate", "1"),
        ("token", &api.get_edit_token().await?),
    ]
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect();

    if bot && matches!(assert, Some(APIAssertType::Bot)) {
        params.insert("bot".to_string(), "1".to_string());
    }
    if let Some(a) = assert {
        params.insert("assert".to_string(), a.to_string());
    };

    let result = api.post_query_api_json(&params).await?;
    match result["edit"]["result"].as_str() {
        Some("Success") => Ok(()),
        _ => {
            let ecode = result["code"].as_str().unwrap_or("<unknown>");
            let einfo = result["info"].as_str().unwrap_or("<unknown>");
            Err(EditPageError::from((String::from(ecode), String::from(einfo))))
        },
    }
}
