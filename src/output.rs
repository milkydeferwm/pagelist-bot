use std::collections::HashSet;
use mediawiki::title::Title;
use mediawiki::api::Api;

pub fn generate_text(list: &HashSet<Title>, api: &Api, before_list_text: &str, list_item_text: &str, between_item_text: &str, after_list_text: &str) -> String {
    let mut output: String = String::new();
    output.push_str(before_list_text);
    let item_str: String = list.iter().map(|t| substitute_template(t, list_item_text, api)).collect::<Vec<String>>().join(between_item_text);
    output.push_str(&item_str);
    output.push_str(after_list_text);
    output
}

fn substitute_template(t: &Title, template: &str, api: &Api) -> String {
    let mut output: String = String::new();
    let mut escape: bool = false;
    for char in template.chars() {
        if escape {
            // only accept $0 (full name), $1 (namespace), $2 (name), $$ ($), $n (\n), $t (\t)
            match char {
                '$' => { output.push('$'); },
                'n' => { output.push('\n'); },
                't' => { output.push('\t'); },
                '0' => { output.push_str(&t.full_pretty(api).unwrap_or("".to_string())); },
                '1' => { output.push_str(t.namespace_name(api).unwrap_or("")); },
                '2' => { output.push_str(t.pretty()); },
                _ => { output.push('$'); output.push(char); },
            }
            escape = false;
        } else {
            if char == '$' {
                escape = true;
            } else {
                output.push(char);
            }
        }
    }
    output
}