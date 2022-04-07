//! API Service holds the MediaWiki API object.

use std::{collections::HashMap, sync::Arc};

use mediawiki::{api::Api, media_wiki_error::MediaWikiError, title::Title};
use serde_json::Value;
use tokio::sync::Mutex;
use crate::types::{LoginCredential, SiteProfile};

#[derive(Debug)]
pub enum APIServiceError {
    NoAPI,
    Client(MediaWikiError),
    Server(Value),
}

// impl std::error::Error for APIServiceError {}
unsafe impl Send for APIServiceError {}

impl From<MediaWikiError> for APIServiceError {
    fn from(e: MediaWikiError) -> Self {
        Self::Client(e)
    }
}

pub struct APIService {
    login: Option<LoginCredential>,
    profile: Option<SiteProfile>,

    api: Option<Api>,
    network_lock: Arc<Mutex<()>>,
    csrf: String,
}

impl APIService {

    /// Creates an APIDelegare instance
    pub fn new() -> Self {
        APIService {
            login: None,
            profile: None,
            api: None,
            network_lock: Arc::new(Mutex::new(())),
            csrf: "".to_string(),
        }
    }

    pub fn setup(&mut self, login: LoginCredential, profile: SiteProfile) {
        self.login = Some(login);
        self.profile = Some(profile);
    }

    /// Send a request via GET
    pub async fn get(&self, params: &HashMap<String, String>) -> Result<Value, APIServiceError> {
        if let Some(api) = &self.api {
            let mut params = params.clone();
            self.param_decorate(&mut params);
            let resp = api.get_query_api_json(&params).await?;
            if let Some(errobj) = resp.get("error") {
                Err(APIServiceError::Server(errobj.clone()))
            } else {
                Ok(resp)
            }
        } else {
            Err(APIServiceError::NoAPI)
        }
    }

    /// Send a request via GET
    pub async fn get_limit(&self, params: &HashMap<String, String>, max: Option<usize>) -> Result<Value, APIServiceError> {
        if let Some(api) = &self.api {
            let mut params = params.clone();
            self.param_decorate(&mut params);
            let resp = api.get_query_api_json_limit(&params, max).await?;
            if let Some(errobj) = resp.get("error") {
                Err(APIServiceError::Server(errobj.clone()))
            } else {
                Ok(resp)
            }
        } else {
            Err(APIServiceError::NoAPI)
        }
    }

    /// Send a request via GET
    pub async fn get_all(&self, params: &HashMap<String, String>) -> Result<Value, APIServiceError> {
        self.get_limit(params, None).await
    }

    /// Send a request via POST
    pub async fn post(&self, params: &HashMap<String, String>) -> Result<Value, APIServiceError> {
        if let Some(api) = &self.api {
            let mut params = params.to_owned();
            self.param_decorate(&mut params);
            let resp = api.post_query_api_json(&params).await?;
            if let Some(errobj) = resp.get("error") {
                Err(APIServiceError::Server(errobj.clone()))
            } else {
                Ok(resp)
            }
        } else {
            Err(APIServiceError::NoAPI)
        }
    }

    pub async fn post_edit(&self, params: &HashMap<String, String>) -> Result<Value, APIServiceError> {
        // Add an bot edit flag to params, if it does not exist
        let mut params = params.to_owned();
        if !params.contains_key("bot") && self.profile.as_ref().unwrap().botflag {
            params.insert("bot".to_string(), "1".to_string());
        }
        self.post(&params).await
    }

    /// Get csrf token
    pub fn csrf(&self) -> String {
        self.csrf.clone()
    }

    pub fn get_lock(&self) -> Arc<Mutex<()>> {
        self.network_lock.clone()
    }

    /// Convert Title object to full pretty title
    pub fn full_pretty(&self, title: &Title) -> Result<Option<String>, APIServiceError> {
        if let Some(api) = &self.api {
            Ok(title.full_pretty(api))
        } else {
            Err(APIServiceError::NoAPI)
        }
    }

    /// Convert Title object to namespace name
    pub fn namespace_name<'a>(&self, title: &Title) -> Result<Option<&'a str>, APIServiceError> {
        if let Some(api) = &self.api {
            Ok(title.namespace_name(api))
        } else {
            Err(APIServiceError::NoAPI)
        }
    }

    #[inline]
    fn extract_base_username(&self) -> String {
        self.login.unwrap().username.split('@').next().unwrap().to_string()
    }

    fn param_decorate(&self, params: &mut HashMap<String, String>) {
        // Add a format to params, if it does not exist
        if !params.contains_key("format") {
            params.insert("format".to_string(), "json".to_string());
        }
        // Add a formatversion to params, if it does not exist
        if !params.contains_key("formatversion") {
            params.insert("formatversion".to_string(), "2".to_string());
        }
        // Add a utf8 to params, if it does not exist
        if !params.contains_key("utf8") {
            params.insert("utf8".to_string(), "1".to_string());
        }
        // Add an assert to params, if it does not exist
        if !params.contains_key("assert") && self.profile.unwrap().assert.is_some() {
            params.insert("assert".to_string(), self.profile.unwrap().assert.unwrap().to_string());
        }
        // Add an assertuser to params, if it does not exist
        if !params.contains_key("assertuser") {
            // extract the part before @
            // notice that @ is in reserved username character list, so that there is no ordinary username that contains @
            params.insert("assertuser".to_string(), self.extract_base_username());
        }
    }

    /// Starts the daemon process. This should only be called once
    pub fn start(&'static self) {
        tokio::spawn(async {
            // API status checker runs every hour
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60 * 60));
            loop {
                interval.tick().await;
                // Require a lock
                let _ = self.network_lock.lock().await;

                if let Some(api) = &mut self.api {
                    // Tries to send a request to check for login status
                    let mut params = api.params_into(&[("action", "query")]);
                    self.param_decorate(&mut params);
                    let response = api.get_query_api_json(&params).await;
                    // Do nothing if a general client-side problem occurs
                    if let Ok(response) = response {
                        if response["error"].as_object().is_some() {
                            // re-login
                            let _ = api.login(&self.login.unwrap().username, &self.login.unwrap().password).await;
                            if let Ok(csrf) = api.get_edit_token().await {
                                self.csrf = csrf;
                            }
                        }
                    }
                } else {
                    // Try to initialize the API object...
                    let api_obj = Api::new(&self.profile.unwrap().api).await;
                    if let Ok(mut api_obj) = api_obj {
                        api_obj.set_maxlag(Some(5));
                        api_obj.set_max_retry_attempts(3);
                        api_obj.set_user_agent(format!("Page List Bot / via User:{}", self.extract_base_username()));
                        let _ = api_obj.login(&self.login.unwrap().username, &self.login.unwrap().password).await;
                        if let Ok(csrf) = api_obj.get_edit_token().await {
                            self.csrf = csrf;
                        }
                        self.api = Some(api_obj);
                    }
                }
            }
        });
    }

}
