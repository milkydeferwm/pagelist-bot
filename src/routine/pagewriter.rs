use std::collections::HashSet;

use futures::future::join_all;
use md5::{Md5, Digest};
use mediawiki::{hashmap, api::NamespaceID, title::Title};
use tokio::sync::Mutex;
use tracing::{event, Level};

use super::{types::OutputFormat, queryexecutor::{QueryExecutor, QueryExecutorError}};
use crate::API_SERVICE;

pub(crate) struct PageWriter<'a> {
    task_id: i64,
    query_executor: Mutex<QueryExecutor>,
    denied_namespace: Option<&'a HashSet<NamespaceID>>,
    outputformat: &'a [OutputFormat],
    header_template_name: &'a str,
}

impl<'a> PageWriter<'a> {

    pub fn new(query_exec: QueryExecutor) -> Self {
        PageWriter {
            task_id: 0,
            query_executor: Mutex::new(query_exec),
            denied_namespace: None,
            outputformat: &[],
            header_template_name: "",
        }
    }

    pub fn set_task_id(mut self, id: i64) -> Self {
        self.task_id = id;
        self
    }

    pub fn set_denied_namespace(mut self, ns: &'a HashSet<NamespaceID>) -> Self {
        self.denied_namespace = Some(ns);
        self
    }

    pub fn set_output_format(mut self, format: &'a [OutputFormat]) -> Self {
        self.outputformat = format;
        self
    }

    pub fn set_header_template_name(mut self, template: &'a str) -> Self {
        self.header_template_name = template;
        self
    }

    fn make_edit_summary(&self, result: &Result<Vec<Title>, QueryExecutorError>) -> String {
        if let Ok(v) = result {
            match v.len() {
                0 => String::from("Update query: empty"),
                1 => String::from("Update query: 1 result"),
                l => format!("Update query: {} results", l)
            }
        } else {
            String::from("Update query: failure")
        }
    }

    fn make_header_content(&self, result: &Result<Vec<Title>, QueryExecutorError>) -> String {
        let status_text = match result {
            Ok(_) => "success",
            Err(e) => match e {
                QueryExecutorError::Timeout => "timeout",
                QueryExecutorError::Parse => "parse",
                QueryExecutorError::Solve => "runtime",
            }
        };
        format!("<noinclude>{{{{subst:{header}|taskid={id}|status={status}}}}}</noinclude>", header=self.header_template_name, id=self.task_id, status=status_text)
    }

    fn substitute_str_template(&self, template: &str, total_num: usize) -> String {
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
    
    async fn substitute_str_template_with_title(&self, template: &str, t: &Title, current_num: usize, total_num: usize) -> String {
        let mut output: String = String::new();
        let mut escape: bool = false;
        for char in template.chars() {
            if escape {
                // only accept $0 (full name), $1 (namespace), $2 (name), $@ (current index), $+ (total size), $$ ($)
                match char {
                    '$' => { output.push('$'); },
                    '0' => { output.push_str(&API_SERVICE.full_pretty(t).await.unwrap_or_else(|_| Some("".to_string())).unwrap_or("".to_string())); },
                    '1' => { output.push_str(&API_SERVICE.namespace_name(t).await.unwrap_or(Some("".to_string())).unwrap_or("".to_string())); },
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

    fn get_md5(&self, text: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(text);
        let result = hasher.finalize();
        hex::encode(result)
    }

    pub async fn start(&self) {
        // Iterate through each page
        for outputformat in self.outputformat {
            // Check whether the page is a redirect or missing
            let params = hashmap![
                "action".to_string() => "query".to_string(),
                "prop".to_string() => "info".to_string(),
                "titles".to_string() => outputformat.target.clone()
            ];
            let page_query = {
                API_SERVICE.get_lock().lock().await;
                API_SERVICE.get(&params).await
            };
            if page_query.is_err() {
                event!(Level::WARN, target = outputformat.target.as_str(), error = ?page_query.unwrap_err(), "cannot fetch page information");
            } else {
                let res = page_query.unwrap();
                let info = res["query"]["pages"].as_array().unwrap()[0].as_object().unwrap();
                if info.get("missing").is_some() {
                    event!(Level::INFO, target = outputformat.target.as_str(), "target page does not exist, skip");
                } else if info.get("redirect").is_some() {
                    event!(Level::INFO, target = outputformat.target.as_str(), "target page is a redirect page, skip");
                } else if let Some(denied_namespace) = self.denied_namespace {
                    if denied_namespace.contains(&info["ns"].as_i64().unwrap()) {
                        event!(Level::INFO, target = outputformat.target.as_str(), "target page is in disallowed namespace, skip");
                    }
                } else {
                    // Not a redirect nor a missing page nor in a denied namespace, continue
                    let mut executor = self.query_executor.lock().await;
                    let result = executor.execute().await;
                    // Prepare contents
                    let summary = self.make_edit_summary(result);
                    let mut content = self.make_header_content(result);
                    content.push_str(&match result {
                        Ok(ls) => {
                            if ls.len() <= 0 {
                                outputformat.empty.clone()
                            } else {
                                let list_size = ls.len();
                                let mut output: String = String::new();
                                output.push_str(&self.substitute_str_template(&outputformat.success.before, list_size));
                                let item_str: String = join_all(ls.iter().enumerate().map(|(idx, t)| async move {
                                    self.substitute_str_template_with_title(&outputformat.success.item, t, idx + 1, list_size).await
                                })).await.join(&self.substitute_str_template(&outputformat.success.between, list_size));
                                output.push_str(&item_str);
                                output.push_str(&self.substitute_str_template(&outputformat.success.after, list_size));
                                output
                            }
                        },
                        Err(_) => outputformat.failure.clone(),
                    });
                    // write to page
                    let md5 = self.get_md5(&content);
                    let params = hashmap![
                        "action".to_string() => "edit".to_string(),
                        "title".to_string() => outputformat.target.clone(),
                        "text".to_string() => content,
                        "summary".to_string() => summary,
                        "md5".to_string() => md5,
                        "nocreate".to_string() => "1".to_string(),
                        "token".to_string() => API_SERVICE.csrf().await
                    ];
                    let edit_result = {
                        API_SERVICE.get_lock().lock().await;
                        API_SERVICE.post_edit(&params).await
                    };
                    if edit_result.is_err() {
                        event!(Level::WARN, target = outputformat.target.as_str(), error = ?edit_result.unwrap_err(), "cannot edit page");
                    } else {
                        event!(Level::WARN, target = outputformat.target.as_str(), "edit page successful");
                    }
                }
            }
        }
    }

}
