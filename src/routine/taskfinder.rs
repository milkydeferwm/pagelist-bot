use std::{collections::{HashMap, HashSet}, sync::Arc};

use mediawiki::{hashmap, api::NamespaceID};
use tokio::{task::JoinHandle, sync::{RwLock, Mutex}};
use tracing::{event, Level};

use crate::API_SERVICE;

use super::types::{SiteConfig, TaskConfig};
use super::taskrunner::TaskRunner;

pub struct TaskFinder {
    on_site_config_location: Mutex<String>,

    global_activate: Arc<RwLock<bool>>,
    global_query_config: Arc<RwLock<TaskConfig>>,
    global_denied_namespace: Arc<RwLock<HashSet<NamespaceID>>>,
    global_output_header: Arc<RwLock<String>>,
    task_map: Mutex<HashMap<i64, TaskRunner>>,

    finderhandle: Mutex<Option<JoinHandle<()>>>,
}

impl TaskFinder {

    pub fn new() -> Self {
        TaskFinder {
            on_site_config_location: Mutex::new("".to_owned()),

            global_activate: Arc::new(RwLock::new(false)),
            global_query_config: Arc::new(RwLock::new(TaskConfig::new())),
            global_denied_namespace: Arc::new(RwLock::new(HashSet::new())),
            global_output_header: Arc::new(RwLock::new(String::new())),

            task_map: Mutex::new(HashMap::new()),
            finderhandle: Mutex::new(None),
        }
    }

    pub async fn set_config_location(&self, config_location: &str) {
        let mut self_config_loc = self.on_site_config_location.lock().await;
        *self_config_loc = config_location.to_owned();
    }

    pub async fn start(&'static self) {
        _ = tokio::task::spawn_blocking(|| self.stop()).await;
        let handle = tokio::spawn(async {
            loop {
                event!(Level::INFO, "task finder starts");
                // fetch on-site config
                let on_site_config: Result<SiteConfig, ()> = {
                    // fetch page content
                    let params = hashmap![
                        "action".to_string() => "query".to_string(),
                        "prop".to_string() => "revisions".to_string(),
                        "titles".to_string() => {
                            let lock = self.on_site_config_location.lock().await;
                            (*lock).clone()
                        },
                        "rvslots".to_string() => "*".to_string(),
                        "rvprop".to_string() => "content".to_string(),
                        "rvlimit".to_string() => "1".to_string()
                    ];
                    let page_content = {
                        API_SERVICE.get_lock().lock().await;
                        API_SERVICE.get(&params).await
                    };
                    if let Ok(page_content) = page_content {
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
                    } else {
                        event!(Level::WARN, error = ?page_content.unwrap_err(), "cannot fetch on-site configuration");
                        Err(())
                    } 
                };
                if let Ok(config) = on_site_config {
                    event!(Level::INFO, "on-site config fetch successful");
                    // update global params
                    {
                        let mut global_activate = self.global_activate.write().await;
                        *global_activate = config.activate;
                    }
                    {
                        let mut global_query_config = self.global_query_config.write().await;
                        *global_query_config = config.default;
                    }
                    {
                        let mut global_denied_namespace = self.global_denied_namespace.write().await;
                        *global_denied_namespace = HashSet::from_iter(config.denyns);
                    }
                    {
                        let mut global_output_header = self.global_output_header.write().await;
                        *global_output_header = config.resultheader;
                    }
                    event!(Level::INFO, "global params update successful");
                    // fetch tasks
                    // so long as we can get site config, there is always an `Api` present in the service
                    let taskdir_title = API_SERVICE.title_new_from_full(&config.taskdir).await.unwrap(); 
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
                        event!(Level::INFO, "task gathered with {} tasks", task_pool.len());
                        {
                            let mut task_map = self.task_map.lock().await;
                            // kill all tasks whose id does not live in the pool
                            (*task_map).retain(|k, _| task_pool.contains(k));
                            // create and start new tasks
                            for id in task_pool {
                                (*task_map).entry(id).or_insert_with(|| {
                                    let mut task_runner: TaskRunner = TaskRunner::new(id, self.global_activate.clone(), self.global_query_config.clone(), self.global_denied_namespace.clone(), self.global_output_header.clone());
                                    task_runner.start();
                                    task_runner
                                });
                            }
                        }
                        event!(Level::INFO, "task pool updated");
                    } else {
                        // we always set the global activated to false to prevent any accidents
                        {
                            let mut global_activate = self.global_activate.write().await;
                            *global_activate = false;
                        }
                        event!(Level::WARN, error = ?tasks.unwrap_err(), "cannot get task list");
                    }
                } else {
                    // we always set the global activated to false to prevent any accidents
                    {
                        let mut global_activate = self.global_activate.write().await;
                        *global_activate = false;
                    }
                }
                // sleep for a fixed 10 minutes
                tokio::time::sleep(tokio::time::Duration::from_secs(10 * 60)).await;
            }
        });
        let mut finderhandle = self.finderhandle.lock().await;
        *finderhandle = Some(handle);
    }

    #[inline]
    fn stop(&self) {
        let mut finderhandle = self.finderhandle.blocking_lock();
        if let Some(handle) = &*finderhandle {
            handle.abort();
        }
        *finderhandle = None;
    }

}

impl Drop for TaskFinder {
    fn drop(&mut self) {
        self.stop();
    }
}
