use std::{collections::{HashMap, HashSet}, sync::Arc};

use mediawiki::{hashmap, api::NamespaceID};
use tokio::{task::JoinHandle, sync::RwLock};
use tracing::{event, Level};

use crate::{types::{SiteConfig, TaskConfig}, API_SERVICE};

use super::taskrunner::TaskRunner;

pub struct TaskFinder {
    on_site_config_location: String,

    global_activate: Arc<RwLock<bool>>,
    global_query_config: Arc<RwLock<TaskConfig>>,
    global_denied_namespace: Arc<RwLock<HashSet<NamespaceID>>>,
    global_output_header: Arc<RwLock<String>>,
    task_map: HashMap<i64, TaskRunner>,

    finderhandle: Option<JoinHandle<()>>,
}

impl TaskFinder {

    pub fn new(config_location: &str) -> Self {
        TaskFinder {
            on_site_config_location: config_location.to_owned(),

            global_activate: Arc::new(RwLock::new(false)),
            global_query_config: Arc::new(RwLock::new(TaskConfig::new())),
            global_denied_namespace: Arc::new(RwLock::new(HashSet::new())),
            global_output_header: Arc::new(RwLock::new(String::new())),

            task_map: HashMap::new(),
            finderhandle: None,
        }
    }

    pub fn start(&'static self) {
        self.stop();
        let handle = tokio::spawn(async {
            loop {
                // fetch on-site config
                let on_site_config: Result<SiteConfig, ()> = {
                    // fetch page content
                    let params = hashmap![
                        "action".to_string() => "query".to_string(),
                        "prop".to_string() => "revisions".to_string(),
                        "titles".to_string() => self.on_site_config_location.clone(),
                        "rvslots".to_string() => "*".to_string(),
                        "rvprop".to_string() => "content".to_string(),
                        "rvlimit".to_string() => "1".to_string()
                    ];
                    let page_content = {
                        API_SERVICE.get_lock().lock().await;
                        API_SERVICE.get(&params).await
                    };
                    if page_content.is_err() {
                        event!(Level::WARN, error = ?page_content.unwrap_err(), "cannot fetch on-site configuration");
                        Err(())
                    } else {
                        let page_content = page_content.unwrap();
                        let page_content_str = page_content["query"]["pages"][0]["revisions"][0]["slots"]["main"]["content"].as_str();
                        if let Some(page_content_str) = page_content_str {
                            let config = serde_json::from_str(page_content_str);
                            if let Ok(config) = config {
                                Ok(config)
                            } else {
                                event!(Level::WARN, content = page_content_str, "cannot parse on-site configuration");
                                Err(())
                            }
                        } else {
                            event!(Level::WARN, response = ?page_content, "cannot find page content in response");
                            Err(())
                        }
                    }
                };
                if let Ok(config) = on_site_config {
                    // update global params
                    {
                        let global_activate = self.global_activate.write().await;
                        *global_activate = config.activate;
                    }
                    {
                        let global_query_config = self.global_query_config.write().await;
                        *global_query_config = config.default;
                    }
                    {
                        let global_denied_namespace = self.global_denied_namespace.write().await;
                        *global_denied_namespace = HashSet::from_iter(config.denyns);
                    }
                    {
                        let global_output_header = self.global_output_header.write().await;
                        *global_output_header = config.resultheader;
                    }
                    // fetch tasks
                    // so long as we can get site config, there is always an `Api` present in the service
                    let taskdir_title = API_SERVICE.title_new_from_full(&config.taskdir).unwrap(); 
                    let params = hashmap![
                        "action".to_string() => "query".to_string(),
                        "prop".to_string() => "info".to_string(),
                        "generator".to_string() => "allpages".to_string(),
                        "gapprefix".to_string() => taskdir_title.pretty().to_string(),
                        "gapnamespace".to_string() => taskdir_title.namespace_id().to_string(),
                        "gaplimit".to_string() => "max".to_string(),
                        "gapfilterredir".to_string() => "nonredirects".to_string()
                    ];
                    let tasks = {
                        API_SERVICE.get_lock().lock().await;
                        API_SERVICE.get_all(&params).await
                    };
                    if let Ok(tasks_result) = tasks {
                        let tasks = tasks_result["query"]["pages"].as_array().unwrap();
                        // gather all tasks
                        let mut task_pool: HashSet<i64> = HashSet::new();
                        for pages in tasks {
                            let pageid = pages["pageid"].as_i64().unwrap();
                            let contentmodel = pages["contentmodel"].as_str().unwrap();
                            if contentmodel == "json" {
                                task_pool.insert(pageid);
                            }
                        }
                        // kill all tasks whose id does not live in the pool
                        self.task_map.retain(|k, v| task_pool.contains(k));
                        // create and start new tasks
                        for id in task_pool {
                            if !self.task_map.contains_key(&id) {
                                let task_runner: TaskRunner = TaskRunner::new(id, self.global_activate.clone(), self.global_query_config.clone(), self.global_denied_namespace.clone(), self.global_output_header.clone());
                                task_runner.start();
                                self.task_map.insert(id, task_runner);
                            }
                        }
                    } else {
                        // we always set the global activated to false to prevent any accidents
                        {
                            let global_activate = self.global_activate.write().await;
                            *global_activate = false;
                        }
                        event!(Level::WARN, error = ?tasks.unwrap_err(), "cannot get task list");
                    }
                } else {
                    // we always set the global activated to false to prevent any accidents
                    {
                        let global_activate = self.global_activate.write().await;
                        *global_activate = false;
                    }
                }
                // sleep for a fixed 10 minutes
                tokio::time::sleep(tokio::time::Duration::from_secs(10 * 60));
            }
        });
        self.finderhandle = Some(handle);
    }

    #[inline]
    fn stop(&self) {
        if let Some(handler) = self.finderhandle {
            handler.abort();
            self.finderhandle = None;
        }
    }

}

impl Drop for TaskFinder {
    fn drop(&mut self) {
        self.stop();
    }
}
