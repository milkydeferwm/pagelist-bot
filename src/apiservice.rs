//! API Service holds the MediaWiki API object.

use std::{collections::HashMap, sync::Arc};

use mediawiki::{api::Api, media_wiki_error::MediaWikiError, title::Title};
use serde_json::Value;
use tokio::{sync::{Mutex, RwLock}, task::JoinHandle};
use tracing::{event, Level, span, Instrument, instrument};
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

impl core::fmt::Display for APIServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAPI => f.write_str("no API object present in the service"),
            Self::Client(e) => e.fmt(f),
            Self::Server(e) => e.fmt(f),
        }
    }
}

#[derive(Debug)]
pub struct APIService {
    login: Mutex<Option<LoginCredential>>,
    profile: Mutex<Option<SiteProfile>>,

    api: RwLock<Option<Api>>,
    network_lock: Arc<Mutex<()>>,
    csrf: RwLock<String>,

    keepalivehandle: Mutex<Option<JoinHandle<()>>>,
}

impl APIService {

    /// Creates an APIDelegare instance
    pub fn new() -> Self {
        APIService {
            login: Mutex::new(None),
            profile: Mutex::new(None),
            api: RwLock::new(None),
            network_lock: Arc::new(Mutex::new(())),
            csrf: RwLock::new("".to_string()),
            keepalivehandle: Mutex::new(None),
        }
    }

    pub async fn setup(&self, login: LoginCredential, profile: SiteProfile) {
        {
            let mut login_lock = self.login.lock().await;
            *login_lock = Some(login);
        }
        {
            let mut profile_lock = self.profile.lock().await;
            *profile_lock = Some(profile);
        }
    }

    /// Send a request via GET
    pub async fn get(&self, params: &HashMap<String, String>) -> Result<Value, APIServiceError> {
        let api = self.api.read().await;
        if let Some(api) = &*api {
            let mut params = params.clone();
            self.param_decorate(&mut params).await;
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
        let api = self.api.read().await;
        if let Some(api) = &*api {
            let mut params = params.clone();
            self.param_decorate(&mut params).await;
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
        let api = self.api.read().await;
        if let Some(api) = &*api {
            let mut params = params.to_owned();
            self.param_decorate(&mut params).await;
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
        if !params.contains_key("bot") && self.profile.lock().await.as_ref().unwrap().botflag {
            params.insert("bot".to_string(), "1".to_string());
        }
        self.post(&params).await
    }

    /// Get csrf token
    pub async fn csrf(&self) -> String {
        let self_csrf = self.csrf.read().await;
        (*self_csrf).clone()
    }

    pub fn get_lock(&self) -> Arc<Mutex<()>> {
        self.network_lock.clone()
    }

    /// Convert Title object to full pretty title
    pub async fn full_pretty(&self, title: &Title) -> Result<Option<String>, APIServiceError> {
        let api = self.api.read().await;
        if let Some(api) = &*api {
            Ok(title.full_pretty(api))
        } else {
            Err(APIServiceError::NoAPI)
        }
    }

    /// Convert Title object to namespace name
    pub async fn namespace_name<'a>(&self, title: &Title) -> Result<Option<String>, APIServiceError> {
        let api = self.api.read().await;
        if let Some(api) = &*api {
            let name = title.namespace_name(api);
            if let Some(name) = name {
                Ok(Some(name.to_owned()))
            } else {
                Ok(None)
            }
        } else {
            Err(APIServiceError::NoAPI)
        }
    }

    /// Create a title from full name
    pub async fn title_new_from_full(&self, title: &str) -> Result<Title, APIServiceError> {
        let api = self.api.read().await;
        if let Some(api) = &*api {
            Ok(Title::new_from_full(title, api))
        } else {
            Err(APIServiceError::NoAPI)
        }
    }

    async fn param_decorate(&self, params: &mut HashMap<String, String>) {
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
        let user_assert = {
            let lock = self.profile.lock().await;
            lock.as_ref().unwrap().assert
        };
        if !params.contains_key("assert") && user_assert.is_some() {
            params.insert("assert".to_string(), user_assert.unwrap().to_string());
        }
        // Add an assertuser to params, if it does not exist
        if !params.contains_key("assertuser") {
            // extract the part before @
            // notice that @ is in reserved username character list, so that there is no ordinary username that contains @
            let user_username = {
                let lock = self.login.lock().await;
                lock.as_ref().unwrap().username.clone()
            };
            params.insert("assertuser".to_string(), user_username.split('@').next().unwrap().to_string());
        }
    }

    #[instrument(target = "API Service", level = "info", name = "API initiator")]
    pub async fn try_init(&'static self) {
        _ = tokio::task::spawn_blocking(|| self.stop()).await;
        event!(Level::INFO, "initiating API");
        // Try to initialize the API object...
        let api_url = {
            let lock = self.profile.lock().await;
            lock.as_ref().unwrap().api.clone()
        };
        let (username, password) = {
            let lock = self.login.lock().await;
            (lock.as_ref().unwrap().username.clone(), lock.as_ref().unwrap().password.clone())
        };
        let api_obj = Api::new(&api_url).await;
        if let Ok(mut api_obj) = api_obj {
            api_obj.set_maxlag(Some(5));
            api_obj.set_max_retry_attempts(3);
            api_obj.set_user_agent(format!("Page List Bot / via User:{}", username.split('@').next().unwrap()));
            let _ = api_obj.login(&username, &password).await;
            if let Ok(csrf) = api_obj.get_edit_token().await {
                let mut self_csrf = self.csrf.write().await;
                *self_csrf = csrf;
            }
            let mut api = self.api.write().await;
            *api = Some(api_obj);
        } else {
            event!(Level::WARN, error = ?api_obj.unwrap_err(), "cannot initiate API");
        }
    }

    /// Starts the daemon process. This should only be called once
    pub async fn start(&'static self) {
        _ = tokio::task::spawn_blocking(|| self.stop()).await;
        let handle = tokio::spawn(async {
            // API status checker runs every hour
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60 * 60));
            loop {
                interval.tick().await;
                event!(Level::INFO, "API checking start");
                // Require a lock
                let _ = self.network_lock.lock().await;
                let mut api = self.api.write().await;
                if let Some(api) = &mut *api {
                    // Tries to send a request to check for login status
                    let params = api.params_into(&[
                        ("action", "query"),
                        ("format", "json"),
                        ("formatversion", "2"),
                        ("assert", &{
                            let lock = self.profile.lock().await;
                            lock.as_ref().unwrap().assert.unwrap().to_string()
                        }),
                        ("assertuser", &{
                            let lock = self.login.lock().await;
                            lock.as_ref().unwrap().username.split('@').next().unwrap().to_string()
                        }),
                    ]);
                    let response = api.get_query_api_json(&params).await;
                    // Do nothing if a general client-side problem occurs
                    if let Ok(response) = response {
                        if response["error"].as_object().is_some() {
                            event!(Level::INFO, "API expired, re-login");
                            // re-login
                            let (username, password) = {
                                let lock = self.login.lock().await;
                                (lock.as_ref().unwrap().username.clone(), lock.as_ref().unwrap().password.clone())
                            };
                            let _ = api.login(&username, &password).await;
                            if let Ok(csrf) = api.get_edit_token().await {
                                let mut self_csrf = self.csrf.write().await;
                                *self_csrf = csrf;
                            }
                        } else {
                            event!(Level::INFO, "API valid");
                        }
                    } else {
                        event!(Level::WARN, error = ?response.unwrap_err(), "cannot check API status");
                    }
                } else {
                    event!(Level::INFO, "API not initiated, initiating");
                    // Try to initialize the API object...
                    let api_url = {
                        let lock = self.profile.lock().await;
                        lock.as_ref().unwrap().api.clone()
                    };
                    let (username, password) = {
                        let lock = self.login.lock().await;
                        (lock.as_ref().unwrap().username.clone(), lock.as_ref().unwrap().password.clone())
                    };
                    let api_obj = Api::new(&api_url).await;
                    if let Ok(mut api_obj) = api_obj {
                        api_obj.set_maxlag(Some(5));
                        api_obj.set_max_retry_attempts(3);
                        api_obj.set_user_agent(format!("Page List Bot / via User:{}", username.split('@').next().unwrap()));
                        let _ = api_obj.login(&username, &password).await;
                        if let Ok(csrf) = api_obj.get_edit_token().await {
                            let mut self_csrf = self.csrf.write().await;
                            *self_csrf = csrf;
                        }
                        *api = Some(api_obj);
                    } else {
                        event!(Level::WARN, error = ?api_obj.unwrap_err(), "cannot initiate API");
                    }
                }
            }
        }.instrument(span!(target: "API Service", Level::INFO, "API checker")));
        let mut keepalivehandle = self.keepalivehandle.lock().await;
        *keepalivehandle = Some(handle);
    }

    #[inline]
    fn stop(&self) {
        let mut keepalivehandle = self.keepalivehandle.blocking_lock();
        if let Some(handle) = &*keepalivehandle {
            event!(Level::INFO, "stopping existing keep alive routine");
            handle.abort();
        }
        *keepalivehandle = None;
    }

}

impl Drop for APIService {
    fn drop(&mut self) {
        self.stop();
    }
}
