use std::sync::Arc;
use std::collections::HashSet;

use mediawiki::{api::Api, title::Title, page::{Page, PageError}};
use plbot_base::{bot::APIAssertType, NamespaceID};
use tokio::{sync::RwLock, time};

use super::types::{TaskStatus, TaskConfig, TaskInfo};
use super::output;

async fn fetch_text_by_id(id: &str, api: Arc<RwLock<Api>>, assert: Option<APIAssertType>) -> Result<String, PageError> {
    let result;
    {
        let api = api.read().await;
        let mut params = api.params_into(&[
            ("utf8", "1"),
            ("action", "query"),
            ("prop", "revisions"),
            ("pageids", id),
            ("rvslots", "*"),
            ("rvprop", "content"),
        ]);
        if let Some(a) = assert {
            params.insert("assert".to_string(), a.to_string());
        };
        result = api
            .get_query_api_json(&params)
            .await
            .map_err(PageError::MediaWiki)?;
    }
    let page = &result["query"]["pages"][0];
    if let Some(slots) = page["revisions"][0]["slots"].as_object() {
        if let Some(the_slot) = {
            slots["main"].as_object().or_else(|| {
                if slots.len() == 1 {
                    slots.values().next().unwrap().as_object() // unwrap OK, length is 1
                } else {
                    None
                }
            })
        } {
            match the_slot["content"].as_str() {
                Some(string) => Ok(string.to_string()),
                None => Err(PageError::BadResponse(result)),
            }
        } else {
            Err(PageError::BadResponse(result))
        }
    } else {
        Err(PageError::BadResponse(result))
    }
}

pub async fn task_runner(id: String, api: Arc<RwLock<Api>>, assert: Option<APIAssertType>, status: Arc<RwLock<TaskStatus>>, default_config: Arc<RwLock<TaskConfig>>, deny_ns: Arc<RwLock<HashSet<NamespaceID>>>, header: Arc<RwLock<String>>) {
    loop {
        // logs the current time
        let now = time::Instant::now();
        // update status running
        println!("[{}] Running task", id);
        {
            let mut status = status.write().await;
            *status = TaskStatus::Running;
        }
        // retrive task page based on page id (aka task id)
        let task: TaskInfo;
        {
            let task_content = fetch_text_by_id(&id, api.clone(), assert).await;
            if task_content.is_err() {
                break;
            }
            let task_content = task_content.unwrap();
            let task_json = serde_json::from_str(&task_content);
            if task_json.is_err() {
                break;
            }
            task = task_json.unwrap();
        }
        // check activate
        if !task.activate {
            break;
        }
        // load configs
        let timeout: u64;
        let limit: i64;
        {
            let default_config = default_config.read().await;
            timeout = task.timeout.unwrap_or(default_config.timeout);
            limit = task.querylimit.unwrap_or(default_config.querylimit);
        }
        let mut content: String = String::new();
        let titles_sorted: Option<Vec<Title>>;
        let query_result;
        // do the query
        println!("[{}] Running query", id);
        {
            let api = api.read().await;
            query_result = time::timeout(time::Duration::from_secs(timeout), parse_and_query(&task.expr, &api, assert, limit)).await;
        }
        // set up output header and output title vector
        {
            let header = header.read().await;
            match query_result {
                Err(_) => {
                    content.push_str(&format!("<noinclude>{{{{{header}|taskid={id}|status=timeout}}}}</noinclude>", header=header, id=id));
                    titles_sorted = None;
                },
                Ok(ref r) => {
                    match r {
                        Err(e) => {
                            content.push_str(&format!("<noinclude>{{{{{header}|taskid={id}|status={reason}}}}}</noinclude>", header=header, id=id, reason=e));
                            titles_sorted = None;
                        }
                        Ok(s) => {
                            content.push_str(&format!("<noinclude>{{{{{header}|taskid={id}|status=success}}}}</noinclude>", header=header, id=id));
                            let mut titles_vec: Vec<Title> = Vec::from_iter(s.iter().cloned());
                            titles_vec.sort_by(|a, b| {
                                match a.namespace_id().cmp(&b.namespace_id()) {
                                    std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
                                    std::cmp::Ordering::Less => std::cmp::Ordering::Less,
                                    std::cmp::Ordering::Equal => a.pretty().cmp(b.pretty()),
                                }
                            });
                            titles_sorted = Some(titles_vec);
                        }
                    }
                },
            }
        }
        // write page one-by-one
        for out in &task.output {
            // set target page
            let target_page: Page;
            {
                let api = api.read().await;
                let target_title: Title = Title::new_from_full(&out.target, &api);
                target_page = Page::new(target_title);
            }
            // check taboo namespace...
            {
                let deny_ns = deny_ns.read().await;
                if deny_ns.contains(&target_page.title().namespace_id()) {
                    continue;
                }
            }
            // set content to write
            let mut content_clone = content.clone();
            {
                let api = api.read().await;
                if let Some(titles) = titles_sorted.as_ref() {
                    let output_text = output::generate_text(titles, &api, &out.before,&out.item, &out.between, &out.after);
                    content_clone.push_str(&output_text);
                }
            }
            // set edit summary
            let summary: String = match titles_sorted {
                None => String::from("Update query: failure"),
                Some(ref c) => format!("Update query: {} result(s)", c.len()),
            };
            // write page
            let write_result;
            {
                let mut api = api.write().await;
                write_result = output::write_page(&target_page, &mut api, content_clone, summary, assert, true).await;
            }
            if write_result.is_err() {
                println!("[{}] Cannot edit target page: {}", id, write_result.unwrap_err());
            } else {
                println!("[{}] Target page edit successful", id);
            }
        }
        // update task status and sleep
        {
            let mut status = status.write().await;
            *status = TaskStatus::Standby;
        }
        time::sleep_until(now + time::Duration::from_secs(task.interval)).await;
    }
    // update task status ready to be purged
    {
        let mut status = status.write().await;
        *status = TaskStatus::Dead;
    }
}

pub async fn parse_and_query(expr: &str, api: &Api, assert: Option<APIAssertType>, default_limit: i64) -> Result<HashSet<Title>, String> {
    let query_inst;
    println!("Running parse");
    let query_result = plbot_parser::parse(expr);
    if query_result.is_err() {
        return Err(String::from("parse"));
    } else {
        query_inst = query_result.unwrap();
    }
    println!("Running solve");
    let solve_result = plbot_solver::solve_api(&query_inst, api, assert, default_limit).await;
    if solve_result.is_err() {
        Err(String::from("runtime"))
    } else {
        Ok(solve_result.unwrap())
    }
}