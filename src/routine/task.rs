use std::sync::Arc;
use std::collections::HashSet;

use mediawiki::{api::Api, title::Title, page::{Page, PageError}};
use crate::types::APIAssertType;
use mediawiki::api::NamespaceID;
use tokio::{sync::RwLock, sync::Mutex, time};
use tracing::{info, warn, error, info_span, Instrument};

use super::types::{TaskStatus, TaskInfo, OutputFormat};
use crate::types::TaskConfig;
use super::output;

async fn fetch_text_by_id(id: &str, api: &Api, assert: Option<APIAssertType>) -> Result<String, PageError> {
    let result;
    {
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
    let page = &result["query"]["pages"][id];
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
            match the_slot["*"].as_str() {
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

async fn fetch_task(id: &str, api: &Api, assert: Option<APIAssertType>) -> Result<TaskInfo, ()> {
    info!(target: "task runner", "load task info");
    info!(target: "task runner", "access task info");
    let task_content = fetch_text_by_id(id, api, assert).await;
    if task_content.is_err() {
        error!(target: "task runner", "access task info failed");
        info!(target: "task runner", "error: {}", task_content.unwrap_err());
        return Err(());
    }
    let task_content = task_content.unwrap();
    info!(target: "task runner", "parse task info");
    let task_json = serde_json::from_str(&task_content);
    if task_json.is_err() {
        error!(target: "task runner", "parse task info failed");
        info!(target: "task runner", "error: {}", task_json.unwrap_err());
        return Err(());
    }
    info!(target: "task runner", "load task info success");
    Ok(task_json.unwrap())
}

async fn write_a_page(out: &OutputFormat, api: &mut Api, assert: Option<APIAssertType>, mut content: String, titles_sorted: Option<&[Title]>, deny_ns: Arc<RwLock<HashSet<NamespaceID>>>, write_lock: Arc<Mutex<()>>) {
    // set target page
    let target_page: Page;
    let target_title: Title = Title::new_from_full(&out.target, api);
    target_page = Page::new(target_title);
    info!(target: "task runner", "write page");
    // check taboo namespace...
    {
        info!(target: "task runner", "checking taboo namespace");
        let deny_ns = deny_ns.read().await;
        info!(target: "task runner", "deny_ns lock acquired");
        if deny_ns.contains(&target_page.title().namespace_id()) {
            warn!(target: "task runner", "write page in forbidden namespace {}, skipping", &target_page.title().namespace_id());
            return;
        }
    }
    // set content to write
    info!(target: "task runner", "generate edit content");
    if let Some(titles) = titles_sorted {
        let output_text = output::generate_text(titles, api, &out.before,&out.item, &out.between, &out.after);
        content.push_str(&output_text);
    }
    // set edit summary
    info!(target: "task runner", "set edit summary");
    let summary: String = match titles_sorted {
        None => String::from("Update query: failure"),
        Some(c) => format!("Update query: {} result(s)", c.len()),
    };
    // write page
    let write_result;
    {
        write_lock.lock().await;
        info!(target: "task runner", "write lock acquired");
        write_result = output::write_page(&target_page, api, content, summary, assert, true).await;
    }
    if write_result.is_err() {
        warn!(target: "task runner", "write page failed");
        info!(target: "task runner", "error: {}", write_result.unwrap_err());
    } else {
        info!(target: "task runner", "write page successful");
    }
}

#[allow(clippy::too_many_arguments)]
async fn task_runner_one_pass(id: &str, api: &mut Api, write_lock: Arc<Mutex<()>>, assert: Option<APIAssertType>, status: Arc<RwLock<TaskStatus>>, default_config: Arc<RwLock<TaskConfig>>, deny_ns: Arc<RwLock<HashSet<NamespaceID>>>, header: Arc<RwLock<String>>) -> Result<time::Instant, ()> {
    // logs the current time
    let now = time::Instant::now();
    info!(target: "task runner", "task start");
    // update status running
    {
        info!(target: "task runner", "update task status");
        let mut status = status.write().await;
        info!(target: "task runner", "task status lock acquired");
        *status = TaskStatus::Running;
        info!(target: "task runner", "status updated to running");
    }
    // retrive task page based on page id (aka task id)
    let task = fetch_task(id, api, assert).instrument(info_span!(target: "task runner", "task")).await?;
    let deadline = now + time::Duration::from_secs(task.interval);
    // check activate
    if !task.activate {
        info!(target: "task runner", "task not activated, skipping");
        return Ok(deadline);
    }
    // load configs
    let timeout: u64;
    let limit: i64;
    {
        info!(target: "task runner", "determine task config");
        let default_config = default_config.read().await;
        timeout = task.timeout.unwrap_or(default_config.timeout);
        limit = task.querylimit.unwrap_or(default_config.querylimit);
        info!(target: "task runner", "task timeout: {} sec, default limit: {}", timeout, limit);
    }
    // do the query
    info!(target: "task runner", "run query");
    let query_result = time::timeout(time::Duration::from_secs(timeout), parse_and_query(&task.expr, api, assert, limit)).instrument(info_span!(target: "task runner", "execute")).await;
    info!(target: "task runner", "run query finish");
    // set up output header and output title vector
    let mut content: String = String::new();
    let titles_sorted: Option<Vec<Title>>;
    {
        let header = header.read().await;
        info!(target: "task runner", "header lock acquired");
        match query_result {
            Err(_) => {
                warn!(target: "task runner", "query timeout");
                content.push_str(&format!("<noinclude>{{{{{header}|taskid={id}|status=timeout}}}}</noinclude>", header=header, id=id));
                titles_sorted = None;
            },
            Ok(ref r) => {
                match r {
                    Err(e) => {
                        warn!(target: "task runner", "query failed");
                        content.push_str(&format!("<noinclude>{{{{{header}|taskid={id}|status={reason}}}}}</noinclude>", header=header, id=id, reason=e));
                        titles_sorted = None;
                    }
                    Ok(s) => {
                        info!(target: "task runner", "query success");
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
        write_a_page(out, api, assert, content.clone(), titles_sorted.as_deref(), deny_ns.clone(), write_lock.clone()).instrument(info_span!(target: "task runner", "output", page=out.target.as_str())).await;
    }
    // update task status and sleep
    {
        info!(target: "task runner", "update task status");
        let mut status = status.write().await;
        info!(target: "task runner", "task status lock acquired");
        *status = TaskStatus::Standby;
        info!(target: "task runner", "status updated to standby");
    }
    info!(target: "task runner", "hibernate for {} sec", task.interval);
    Ok(deadline)
}

#[allow(clippy::too_many_arguments)]
pub async fn task_runner(id: String, mut api: Api, write_lock: Arc<Mutex<()>>, assert: Option<APIAssertType>, status: Arc<RwLock<TaskStatus>>, default_config: Arc<RwLock<TaskConfig>>, deny_ns: Arc<RwLock<HashSet<NamespaceID>>>, header: Arc<RwLock<String>>) {
    while let Ok(ddl) = task_runner_one_pass(&id, &mut api, write_lock.clone(), assert, status.clone(), default_config.clone(), deny_ns.clone(), header.clone()).instrument(info_span!(target: "task runner", parent: None, "task", task=id.as_str())).await {
        // now, hibernate
        time::sleep_until(ddl).await;
    }
    // update task status ready to be purged
    {
        info!(target: "task runner", "update task status");
        let mut status = status.write().await;
        info!(target: "task runner", "task status lock acquired");
        *status = TaskStatus::Dead;
        info!(target: "task runner", "status updated to dead");
    }
}

pub async fn parse_and_query(expr: &str, api: &Api, assert: Option<APIAssertType>, default_limit: i64) -> Result<HashSet<Title>, String> {
    let query_inst;
    info!(target: "task runner", "parse expression");
    let query_result = crate::parser::parse(expr);
    if query_result.is_err() {
        warn!(target: "task runner", "parse failure");
        info!(target: "task runner", "error: {}", query_result.unwrap_err());
        return Err(String::from("parse"));
    } else {
        query_inst = query_result.unwrap();
    }
    info!(target: "task runner", "solve expression");
    let solve_result = crate::solver::solve_api(&query_inst, api, assert, default_limit).await;
    if solve_result.is_err() {
        warn!(target: "task runner", "solve failure");
        info!(target: "task runner", "error: {}", solve_result.unwrap_err());
        Err(String::from("solve"))
    } else {
        Ok(solve_result.unwrap())
    }
}