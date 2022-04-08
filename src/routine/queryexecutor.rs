use mediawiki::title::Title;

use crate::{types::TaskConfig, API_SERVICE};

pub enum QueryExecutorError {
    Timeout,
    Parse,
    Solve,
}

pub struct QueryExecutor {
    query: String,
    querylimit: TaskConfig,

    result: Option<Result<Vec<Title>, QueryExecutorError>>,
}

impl QueryExecutor {
    pub fn new(query: &str, limit: &TaskConfig) -> Self {
        QueryExecutor { query: query.to_string(), querylimit: limit.clone(), result: None }
    }

    pub async fn execute(&mut self) -> &Result<Vec<Title>, QueryExecutorError> {
        if self.result.is_none() {
            // run the query first
            let parse_result = crate::parser::parse(&self.query);
            if parse_result.is_err() {
                // warn!(target: "task runner", "parse failure");
                // info!(target: "task runner", "error: {}", query_result.unwrap_err());
                self.result = Some(Err(QueryExecutorError::Parse));
            } else {
                let query_inst = parse_result.unwrap();
                let query_result = {
                    API_SERVICE.get_lock().lock().await;
                    tokio::time::timeout(tokio::time::Duration::from_secs(self.querylimit.timeout), crate::solver::solve_api(&query_inst, self.querylimit.querylimit)).await
                };

                if query_result.is_err() {
                    self.result = Some(Err(QueryExecutorError::Timeout));
                } else {
                    let query_result = query_result.unwrap();
                    if query_result.is_err() {
                        self.result = Some(Err(QueryExecutorError::Solve));
                    } else {
                        let query_result = query_result.unwrap();
                        let mut titles_vec = Vec::from_iter(query_result.into_iter());
                        titles_vec.sort_by(|a, b| {
                            match a.namespace_id().cmp(&b.namespace_id()) {
                                std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
                                std::cmp::Ordering::Less => std::cmp::Ordering::Less,
                                std::cmp::Ordering::Equal => a.pretty().cmp(b.pretty()),
                            }
                        });
                        self.result = Some(Ok(titles_vec));
                    }
                }
            }
        }
        self.result.as_ref().unwrap()
    }
}
