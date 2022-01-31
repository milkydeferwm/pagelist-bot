use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use plbot_base::NamespaceID;
use plbot_base::bot::APIAssertType;
use serde_json::Map;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time;
use mediawiki::{page::Page, title::Title, api::Api};

use super::task::task_runner;
use super::types::*;

struct TaskFrame {
    pub delete: bool,
    pub status: Arc<RwLock<TaskStatus>>,
    pub handle: Option<JoinHandle<()>>,
}

pub async fn task_daemon(config_page_name: String, api: Api, assert: Option<APIAssertType>) {
    let config_page: Page;
    let config_title = Title::new_from_full(&config_page_name, &api);
    config_page = Page::new(config_title);

    let default_config: Arc<RwLock<TaskConfig>> = Arc::new(RwLock::new(TaskConfig::new()));
    let output_header: Arc<RwLock<String>> = Arc::new(RwLock::new(String::new()));
    let deny_ns: Arc<RwLock<HashSet<NamespaceID>>> = Arc::new(RwLock::new(HashSet::new()));

    // set up a task info hashmap
    // use the pageid as key, this enables us to track a task page after moving
    let mut taskmap: HashMap<String, TaskFrame> = HashMap::new();
    let write_lock: Arc<Mutex<bool>> = Arc::new(Mutex::new(true));

    loop {
        // logs the current time
        let now = time::Instant::now();
        // fetch site configuration
        let config: SiteConfig;
        {
            let config_raw = config_page.text(&api).await;
            if config_raw.is_err() {
                break;
            }
            let config_raw = config_raw.unwrap();
            let config_json = serde_json::from_str(&config_raw);
            if config_json.is_err() {
                break;
            }
            config = config_json.unwrap();
        }
        // update config
        {
            let mut default_config = default_config.write().await;
            *default_config = config.default.clone();
        }
        {
            let mut output_header = output_header.write().await;
            *output_header = config.resultheader.clone();
        }
        {
            let mut deny_ns = deny_ns.write().await;
            *deny_ns = HashSet::from_iter(config.denyns.iter().cloned());
        }

        // use a never-loop to emulate goto
        #[allow(clippy::never_loop)]
        loop {
            // if not activated, kill all tasks and sleep
            if !config.activate {
                for (_, frame) in &taskmap {
                    if let Some(hand) = &frame.handle {
                        hand.abort();
                    }
                }
                taskmap.clear();
                break;
            }
            // fetch a list of tasks
            let tasklist: Map<String, serde_json::Value>;
            {
                let taskdir_title = Title::new_from_full(&config.taskdir, &api);
                let mut params = api.params_into(&[
                    ("utf8", "1"),
                    ("action", "query"),
                    ("generator", "allpages"),
                    ("gapprefix", taskdir_title.pretty()),
                    ("gapnamespace", mediawiki::api::NamespaceID::to_string(&taskdir_title.namespace_id()).as_str()),
                    ("gaplimit", "max"),
                    ("gapfilterredir", "nonredirects"),
                ]);
                if let Some(a) = assert {
                    params.insert("assert".to_string(), a.to_string());
                };
                let res = api.get_query_api_json_all(&params).await;
                if res.is_err() {
                    eprintln!("Warning: Cannot fetch task list");
                    break;
                }
                let tasklist_v = res.unwrap();
                let tasklist_v = tasklist_v["query"]["pages"].as_object();
                if tasklist_v.is_none() {
                    break;
                }
                tasklist = tasklist_v.unwrap().to_owned();
            }
            // mark all tasks as delete
            for (_, v) in taskmap.iter_mut() {
                v.delete = true;
            }
            // dispatch all tasks
            for (task_pageid, task_page_obj) in tasklist.iter() {
                println!("Find task: {}", task_pageid);
                // if task page title is not a json, ignore it
                if let Some(s) = task_page_obj["title"].as_str() {
                    if !s.ends_with(".json") {
                        continue;
                    }
                } else {
                    continue;
                }
                // if the `task_pageid` does not exist in the taskmap, create it, dispatch it, log it
                let thistask = taskmap.get(task_pageid);
                if thistask.is_none() {
                    let mut newtaskframe: TaskFrame = TaskFrame{ delete: true, status: Arc::new(RwLock::new(TaskStatus::Standby)), handle: None };
                    newtaskframe.handle = Some(tokio::spawn(task_runner(task_pageid.clone(), api.clone(), write_lock.clone(), assert, newtaskframe.status.clone(), default_config.clone(), deny_ns.clone(), output_header.clone())));
                    taskmap.insert(task_pageid.clone(), newtaskframe);
                }
                let thistask = taskmap.get_mut(task_pageid).unwrap();
                {
                    let taskstatus = thistask.status.read().await;
                    if *taskstatus != TaskStatus::Dead {
                        thistask.delete = false;
                    }
                }
            }
            // purge dead tasks
            taskmap.retain(|_, v| !v.delete);
            break;
        }
        // now, hibernate
        println!("Goes hibernate");
        time::sleep_until(now + time::Duration::from_secs(config.interval)).await;
    }
    // cleanup, that is, kill all tasks
    for (_, frame) in &taskmap {
        if let Some(hand) = &frame.handle {
            hand.abort();
        }
    }
}
