use std::str::FromStr;
use std::{sync::Arc, collections::HashSet};

use mediawiki::api::NamespaceID;
use mediawiki::hashmap;
use tokio::{task::JoinHandle, sync::RwLock};
use tracing::{event, Level};

use crate::{types::TaskConfig, API_SERVICE};

use super::types::TaskInfo;
use super::{pagewriter::PageWriter, queryexecutor::QueryExecutor};

struct TaskRunner {
    id: String,
    global_activate: Arc<RwLock<bool>>,
    global_query_config: Arc<RwLock<TaskConfig>>,
    global_denied_namespace: Arc<RwLock<HashSet<NamespaceID>>>,
    global_output_header: Arc<RwLock<String>>,

    runnerhandler: Option<JoinHandle<()>>,
}

impl TaskRunner {

    pub fn new(
        id: &str,
        global_activate: Arc<RwLock<bool>>,
        global_query_config: Arc<RwLock<TaskConfig>>,
        global_denied_namespace: Arc<RwLock<HashSet<NamespaceID>>>,
        global_output_header: Arc<RwLock<String>>
    ) -> Self {
        TaskRunner {
            id: id.to_string(),
            global_activate,
            global_query_config,
            global_denied_namespace,
            global_output_header,
            runnerhandler: None,
        }
    }

    pub fn start(&self) {
        self.stop();
        let handler: JoinHandle<()> = {
            let id = self.id.clone();
            let global_activate = self.global_activate.clone();
            let global_query_config = self.global_query_config.clone();
            let global_denied_namespace = self.global_denied_namespace.clone();
            let global_output_header = self.global_output_header.clone();

            tokio::spawn(async move {
                // used in first run; we need to align the task runner to cron
                let mut aligned_to_cron: bool = false;
                loop {
                    // fetch task information
                    let task: Result<TaskInfo, ()> = {
                        // fetch page content
                        let params = hashmap![
                            "action".to_string() => "query".to_string(),
                            "prop".to_string() => "revisions".to_string(),
                            "pageids".to_string() => id.clone(),
                            "rvslots".to_string() => "*".to_string(),
                            "rvprop".to_string() => "content".to_string(),
                            "rvlimit".to_string() => "1".to_string()
                        ];
                        let page_content = {
                            API_SERVICE.get_lock().lock().await;
                            API_SERVICE.get(&params).await
                        };
                        if page_content.is_err() {
                            event!(Level::WARN, error = ?page_content.unwrap_err(), "cannot fetch task content");
                            Err(())
                        } else {
                            let page_content = page_content.unwrap();
                            let page_content_str = page_content["query"]["pages"][0]["revisions"][0]["slots"]["main"]["content"].as_str();
                            if let Some(page_content_str) = page_content_str {
                                let task = serde_json::from_str(page_content_str);
                                if let Ok(task) = task {
                                    Ok(task)
                                } else {
                                    event!(Level::WARN, content = page_content_str, "cannot parse task information");
                                    Err(())
                                }
                            } else {
                                event!(Level::WARN, response = ?page_content, "cannot find page content in response");
                                Err(())
                            }
                        }
                    };
                    if let Ok(task) = task {
                        let global_activated = {
                            *global_activate.read().await
                        };
                        // run the task only if bot is globally activated, the task is activated, and the runner is aligned to cron
                        if global_activated && task.activate && aligned_to_cron {
                            let task_config = {
                                let value = global_query_config.read().await;
                                let timeout = task.timeout.unwrap_or(value.timeout);
                                let limit = task.querylimit.unwrap_or(value.querylimit);
                                TaskConfig { timeout, querylimit: limit }
                            };
                            let denied_ns = {
                                let value = global_denied_namespace.read().await;
                                value.clone()
                            };
                            let output_header = {
                                let value = global_output_header.read().await;
                                value.clone()
                            };
                            let writer = PageWriter::new(QueryExecutor::new(&task.expr, &task_config))
                                .set_task_id(&id)
                                .set_output_format(&task.output)
                                .set_denied_namespace(&denied_ns)
                                .set_header_template_name(&output_header);
                            writer.start().await;
                        }
                        // sleep until next cron time
                        let schedule = cron::Schedule::from_str(&task.cron);
                        if let Ok(schedule) = schedule {
                            let waketime = schedule.upcoming(chrono::Utc).next().unwrap();
                            let duration = waketime.signed_duration_since(chrono::Utc::now()).to_std().unwrap();
                            aligned_to_cron = true;
                            tokio::time::sleep(duration);
                        } else {
                            event!(Level::WARN, cron = task.cron.as_str(), error = ?schedule.unwrap_err(), "cannot parse cron specification");
                            // need to re-align later
                            aligned_to_cron = false;
                            // retry in 10 minutes
                            tokio::time::sleep(tokio::time::Duration::from_secs(10 * 60));
                        }
                        todo!();
                    } else {
                        // need to re-align later
                        aligned_to_cron = false;
                        // retry in 10 minutes
                        tokio::time::sleep(tokio::time::Duration::from_secs(10 * 60));
                    }
                }
            })
        };
        self.runnerhandler = Some(handler);
    }

    #[inline]
    fn stop(&self) {
        if let Some(handler) = self.runnerhandler {
            handler.abort();
            self.runnerhandler = None;
        }
    }

}

impl Drop for TaskRunner {
    fn drop(&mut self) {
        self.stop();
    }
}