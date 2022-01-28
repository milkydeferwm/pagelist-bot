// This is the bot's main routine

use mediawiki::api::Api;
use mediawiki::title::Title;
use mediawiki::page::Page;
use plbot_base::bot::{APIAssertType, LoginCredential, SiteProfile, SiteConfig, TaskInfo, TaskConfig};
use tokio::time;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::collections::{HashMap, HashSet};

use crate::output;

struct TaskFrame {
    pub alive: bool,
    pub last_fire: Option<tokio::time::Instant>,
    pub mutex: Arc<Mutex<bool>>,
}

pub async fn task_daemon(login: LoginCredential, profile: SiteProfile) {
    let mut api: Api = Api::new(&profile.api).await.expect("cannot access target MediaWiki instance");
    api.set_maxlag(Some(5));
    api.set_max_retry_attempts(3);
    api.set_user_agent(format!("Page List Bot / via User:{}", login.username));
    api.login(login.username, login.password).await.expect("cannot log in");

    let config_title = Title::new_from_full(&profile.config, &api);
    let config_page = Page::new(config_title);

    // set up a task info hashmap
    // use the pageid as key, this enables us to track a task page after moving
    let mut taskmap: HashMap<String, TaskFrame> = HashMap::new();

    loop {
        // logs the current time
        let now = time::Instant::now();
        // fetch on-site json config file
        let config = config_page.text(&api).await.expect("cannot access on-site configuration");
        let config: SiteConfig = serde_json::from_str(&config).expect("cannot parse on-site configuration");
        
        let taskdir_title = Title::new_from_full(&config.taskdir, &api);
        // fetch a list of tasks
        let mut params = api.params_into(&[
            ("utf8", "1"),
            ("action", "query"),
            ("generator", "allpages"),
            ("gapprefix", taskdir_title.pretty()),
            ("gapnamespace", mediawiki::api::NamespaceID::to_string(&taskdir_title.namespace_id()).as_str()),
            ("gaplimit", "max"),
            ("gapfilterredir", "nonredirects"),
        ]);
        if let Some(a) = profile.assert {
            params.insert("assert".to_string(), a.to_string());
        };
        let res = api.get_query_api_json_all(&params).await;
        if res.is_err() {
            eprintln!("Warning: Cannot fetch task list");
        } else {
            let tasklist = res.unwrap();
            let tasklist = tasklist["query"]["pages"].as_object();
            if let Some(tasklist) = tasklist {
                // set every task in taskmap to false, this enables us to remove any task that no longer exists
                for (_, task_item) in taskmap.iter_mut() {
                    task_item.alive = false;
                }
                // poll for tasks that need to be run
                for (task_pageid, task_page_title) in tasklist.iter() {
                    println!("Find task: {}", task_pageid);
                    // if the `task_pageid` does not exist in the taskmap, create it
                    let thistask = taskmap.get(task_pageid);
                    if thistask.is_none() {
                        let newtaskframe: TaskFrame = TaskFrame{ alive: true, last_fire: None, mutex: Arc::new(Mutex::new(false)) };
                        taskmap.insert(task_pageid.clone(), newtaskframe);
                    }
                    let thistask = taskmap.get_mut(task_pageid).unwrap();
                    thistask.alive = true;
                    {
                        let probe_result = thistask.mutex.try_lock();
                        match probe_result {
                            Err(_) => continue, // task still occupied, ignores this loop
                            _ => (),
                        }
                    }
                    // query for the task, to check for fire time
                    let tasktitle = Title::new_from_api_result(task_page_title);
                    let taskpage = Page::new(tasktitle);
                    let task = taskpage.text(&api).await;
                    if task.is_err() {
                        continue; // page retrival fail, passively ignores this loop
                    }
                    let task = task.unwrap();
                    let task: Result<TaskInfo, serde_json::Error> = serde_json::from_str(&task);
                    if task.is_err() {
                        continue; // page parse fail, passively ignores this loop
                    }
                    let task = task.unwrap();
                    if let Some(last_run) = &thistask.last_fire {
                        if *last_run + time::Duration::from_secs(task.interval) > now {
                            continue; // don't activate now
                        }
                    }
                    // activate the task, update the `last_fire` time
                    thistask.last_fire = Some(now);
                    // duplicate an instance of api
                    let api_dup = api.clone();
                    tokio::spawn(task_runner(task_pageid.clone(), task, api_dup, profile.assert, thistask.mutex.clone(), config.default, config.resultheader.clone()));
                }
                // remove any task info page that no longer exists (i.e. deleted)
                taskmap.retain(|_, v| v.alive);
            }
        }
        // now, hibernate
        println!("Goes hibernate");
        time::sleep_until(now + time::Duration::from_secs(config.interval)).await;
    }
}

/// This function handles the writing to the page.
pub async fn task_runner(id: String, task: TaskInfo, mut api: Api, assert: Option<APIAssertType>, lock: Arc<Mutex<bool>>, default_config: TaskConfig, resultheader: String) {
    println!("Running task {}", id);
    // we are going to occupy the lock throughout the task
    let _ = lock.lock();
    println!("Lock acquired");
    
    // prepare to do the work, with timeout
    let timeout = task.timeout.unwrap_or(default_config.timeout);
    let limit = task.querylimit.unwrap_or(default_config.querylimit);
    let mut content: String = String::new();
    let titles_sorted: Option<Vec<Title>>;
    println!("Running query");
    let query_result = time::timeout(time::Duration::from_secs(timeout), parse_and_query(&task.expr, &api, assert, limit)).await;
    match query_result {
        Err(_) => {
            content.push_str(&format!("<noinclude>{{{{{header}|taskid={id}|status=timeout}}}}</noinclude>", header=resultheader, id=id));
            titles_sorted = None;
        },
        Ok(ref r) => {
            match r {
                Err(e) => {
                    content.push_str(&format!("<noinclude>{{{{{header}|taskid={id}|status={reason}}}}}</noinclude>", header=resultheader, id=id, reason=e));
                    titles_sorted = None;
                }
                Ok(s) => {
                    content.push_str(&format!("<noinclude>{{{{{header}|taskid={id}|status=success}}}}</noinclude>", header=resultheader, id=id));
                    let mut titles_vec: Vec<Title> = Vec::from_iter(s.iter().cloned());
                    titles_vec.sort_by(|a, b| {
                        if a.namespace_id() < b.namespace_id() {
                            std::cmp::Ordering::Less
                        } else if a.namespace_id() > b.namespace_id() {
                            std::cmp::Ordering::Greater
                        } else {
                            a.pretty().cmp(&b.pretty())
                        }
                    });
                    titles_sorted = Some(titles_vec);
                }
            }
        },
    }
    
    for out in &task.output {
        let target_title: Title = Title::new_from_full(&out.target, &api);
        let target_page = Page::new(target_title);
        let mut content_clone = content.clone();
        if let Some(titles) = titles_sorted.as_ref() {
            let output_text = output::generate_text(titles, &api, &out.before,&out.item, &out.between, &out.after);
            content_clone.push_str(&output_text);
        }
        let summary: String = match titles_sorted {
            None => String::from("Update query: failure"),
            Some(ref c) => format!("Update query: {} result(s)", c.len()),
        };
        let write_result = output::write_page(&target_page, &mut api, content_clone, summary, assert, true).await;
        if write_result.is_err() {
            println!("Cannot edit target page: {}", write_result.unwrap_err());
        } else {
            println!("Target page edit successful");
        }
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
    let solve_result = plbot_solver::solve_api(&query_inst, &api, assert, default_limit).await;
    if solve_result.is_err() {
        return Err(String::from("runtime"));
    } else {
        return Ok(solve_result.unwrap());
    }
}
