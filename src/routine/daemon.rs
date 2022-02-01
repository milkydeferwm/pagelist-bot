use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use plbot_base::NamespaceID;
use plbot_base::bot::APIAssertType;
use serde_json::Map;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time;
use tracing::{debug_span, info_span, debug, info, warn, error, Instrument};
use mediawiki::{page::Page, title::Title, api::Api};

use super::task::task_runner;
use super::types::*;

struct TaskFrame {
    pub delete: bool,
    pub status: Arc<RwLock<TaskStatus>>,
    pub handle: Option<JoinHandle<()>>,
}

async fn fetch_config(config_page: &Page, api: &Api) -> Result<SiteConfig, ()> {
    info!(target: "task daemon", "load on-site config");
    debug!(target: "task daemon", "access on-site config");
    let config_raw = config_page.text(api).await;
    if config_raw.is_err() {
        error!(target: "task daemon", "access on-site config failed");
        debug!(target: "task daemon", "error: {}", config_raw.unwrap_err());
        return Err(());
    }
    let config_raw = config_raw.unwrap();
    let config_json = serde_json::from_str(&config_raw);
    if config_json.is_err() {
        error!(target: "task daemon", "parse on-site config failed");
        debug!(target: "task daemon", "error: {}", config_json.unwrap_err());
        return Err(());
    }
    Ok(config_json.unwrap())
}

async fn fetch_tasklist(config: &SiteConfig, api: &Api, assert: Option<APIAssertType>) -> Result<Map<String, serde_json::Value>, ()> {
    info!(target: "task daemon", "load task list");
    debug!(target: "task daemon", "build query params");
    debug!(target: "task daemon", "task dir: {}", &config.taskdir);
    let taskdir_title = Title::new_from_full(&config.taskdir, api);
    let mut params = api.params_into(&[
        ("utf8", "1"),
        ("action", "query"),
        ("generator", "allpages"),
        ("gapprefix", taskdir_title.pretty()),
        ("gapnamespace", NamespaceID::to_string(&taskdir_title.namespace_id()).as_str()),
        ("gaplimit", "max"),
        ("gapfilterredir", "nonredirects"),
    ]);
    if let Some(a) = assert {
        params.insert("assert".to_string(), a.to_string());
    };
    debug!(target: "task daemon", "params: {:?}", &params);

    debug!(target: "task daemon", "access remote MediaWiki API");
    let res = api.get_query_api_json_all(&params).await;
    if res.is_err() {
        warn!(target: "task daemon", "cannot access remote MediaWiki API");
        debug!(target: "task daemon", "error: {}", res.unwrap_err());
        return Err(());
    }
    let tasklist_v = res.unwrap();
    let tasklist_vv = tasklist_v["query"]["pages"].as_object();
    if tasklist_vv.is_none() {
        warn!(target: "task daemon", "cannot get task list");
        debug!(target: "task daemon", "response: {}", tasklist_v);
        return Err(());
    }
    info!(target: "task daemon", "load task list success");
    Ok(tasklist_vv.unwrap().to_owned())
}

#[allow(clippy::too_many_arguments)]
async fn task_daemon_one_pass(config_page: &Page, api: &Api, assert: Option<APIAssertType>, taskmap: &mut HashMap::<String, TaskFrame>, default_config: Arc<RwLock<TaskConfig>>, output_header: Arc<RwLock<String>>, deny_ns: Arc<RwLock<HashSet<NamespaceID>>>, write_lock: Arc<Mutex<()>>) -> Result<tokio::time::Instant, ()> {
    // logs the current time
    let now = time::Instant::now();
    info!(target: "task daemon", "task daemon starts");
    // fetch site configuration
    let config = fetch_config(config_page, api).instrument(info_span!(target: "task daemon", "config fetch")).await?;
    // determine next wakeup time
    let deadline = now + time::Duration::from_secs(config.interval);
    // update config
    async {
        info!(target: "task daemon", "update shared config");
        {
            debug!(target: "task daemon", "update default_config");
            let mut default_config = default_config.write().await;
            debug!(target: "task daemon", "default_config lock acquired");
            *default_config = config.default.clone();
        }
        {
            debug!(target: "task daemon", "update output_header");
            let mut output_header = output_header.write().await;
            debug!(target: "task daemon", "output_header lock acquired");
            *output_header = config.resultheader.clone();
        }
        {
            debug!(target: "task daemon", "update deny_ns");
            let mut deny_ns = deny_ns.write().await;
            debug!(target: "task daemon", "deny_ns lock acquired");
            *deny_ns = HashSet::from_iter(config.denyns.iter().cloned());
        }
    }.instrument(info_span!(target: "task daemon", "config update")).await;

    // use a never-loop to emulate goto
    #[allow(clippy::never_loop)]
    loop {
        // if not activated, kill all tasks and sleep
        if !config.activate {
            info!(target: "task daemon", "daemon not activated, kill all tasks and sleep");
            for (taskid, frame) in taskmap.iter() {
                if let Some(hand) = &frame.handle {
                    info!(target: "task daemon", "kill task {}", taskid);
                    hand.abort();
                }
            }
            debug!(target: "task daemon", "clear task map");
            taskmap.clear();
            return Ok(deadline);
        }
        // fetch a list of tasks
        let tasklist = fetch_tasklist(&config, api, assert).instrument(info_span!(target: "task daemon", "task list")).await;
        if tasklist.is_err() {
            return Ok(deadline);
        }
        let tasklist = tasklist.unwrap();
        // update taskmap
        async {
            // mark all tasks as delete
            for (_, v) in taskmap.iter_mut() {
                v.delete = true;
            }
            // dispatch all tasks
            info!(target: "task daemon", "dispatch tasks");
            for (task_pageid, task_page_obj) in tasklist.iter() {
                info!(target: "task daemon", "found task: {}", task_pageid);
                // if task page title is not a json, ignore it
                if let Some(s) = task_page_obj["title"].as_str() {
                    if !s.ends_with(".json") {
                        warn!(target: "task daemon", "task id {} ignored because page title does not end with \".json\"", task_pageid);
                        info!(target: "task daemon", "actual title: {}", s);
                        continue;
                    }
                } else {
                    warn!(target: "task daemon", "task id {} ignored because cannot extract page title", task_pageid);
                    info!(target: "task daemon", "actual API response: {}", task_page_obj);
                    continue;
                }
                // if the `task_pageid` does not exist in the taskmap, create it, dispatch it, log it
                let thistask = taskmap.get(task_pageid);
                if thistask.is_none() {
                    info!(target: "task daemon", "task id {} not exist in task map", task_pageid);
                    info!(target: "task daemon", "create task entry for {}", task_pageid);
                    let mut newtaskframe: TaskFrame = TaskFrame{ delete: true, status: Arc::new(RwLock::new(TaskStatus::Standby)), handle: None };
                    newtaskframe.handle = Some(tokio::spawn(task_runner(task_pageid.clone(), api.clone(), write_lock.clone(), assert, newtaskframe.status.clone(), default_config.clone(), deny_ns.clone(), output_header.clone())));
                    taskmap.insert(task_pageid.clone(), newtaskframe);
                    info!(target: "task daemon", "dispatch task id {} success", task_pageid);
                }
                let thistask = taskmap.get_mut(task_pageid).unwrap();
                {
                    let taskstatus = thistask.status.read().await;
                    if *taskstatus != TaskStatus::Dead {
                        thistask.delete = false;
                    }
                }
            }
            info!(target: "task daemon", "dispatch tasks complete");
            // purge dead tasks
            info!(target: "task daemon", "purge dead tasks");
            taskmap.retain(|_, v| !v.delete);
        }.instrument(info_span!(target: "task daemon", "dispatch")).await;
        break;
    }
    info!(target: "task daemon", "hibernate for {} sec", config.interval);
    Ok(deadline)
}

pub async fn task_daemon(config_page_name: String, api: Api, assert: Option<APIAssertType>) {
    let config_page = debug_span!(target: "task daemon", parent: None, "config").in_scope(|| {
        debug!(target: "task daemon", "on-site global config page: {}", &config_page_name);
        let config_title = Title::new_from_full(&config_page_name, &api);
        Page::new(config_title)
    });

    let (default_config, output_header, deny_ns) = debug_span!(target: "task daemon", parent: None, "resource").in_scope(|| {
        debug!(target: "task daemon", "initializing shared resources");
        (
            Arc::new(RwLock::new(TaskConfig::new())),
            Arc::new(RwLock::new(String::new())),
            Arc::new(RwLock::new(HashSet::<NamespaceID>::new())),
        )
    });

    // set up a task info hashmap
    // use the pageid as key, this enables us to track a task page after moving
    let mut taskmap = debug_span!(target: "task daemon", parent: None, "task map").in_scope(|| {
        debug!(target: "task daemon", "initializing task map");
        HashMap::<String, TaskFrame>::new()
    });

    let write_lock = debug_span!(target: "task daemon", parent: None, "api lock").in_scope(|| {
        debug!(target: "task daemon", "initializing API locks");
        Arc::new(Mutex::new(()))
    });

    while let Ok(ddl) = task_daemon_one_pass(&config_page, &api, assert, &mut taskmap, default_config.clone(), output_header.clone(), deny_ns.clone(), write_lock.clone()).instrument(info_span!(target: "task daemon", parent: None, "daemon")).await {
        // now, hibernate
        time::sleep_until(ddl).await;
    }
    // cleanup, that is, kill all tasks
    info_span!(target: "task daemon", parent: None, "clean up").in_scope(|| {
        info!(target: "task daemon", "kill all tasks");
        for (taskid, frame) in &taskmap {
            if let Some(hand) = &frame.handle {
                info!(target: "task daemon", "kill task {}", taskid);
                hand.abort();
            }
        }
    });
}
