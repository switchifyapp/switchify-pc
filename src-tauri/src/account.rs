//! Switchify account sign-in with an emailed code, shared with the Android
//! app's Supabase project. Requests go from Rust so the webview's content
//! security policy stays closed. The refresh token lives in the OS keychain;
//! access tokens stay in memory. Neither, nor the code, is ever logged or
//! sent to the webview.

use crate::protocol::PairingIntent;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const KEYRING_SERVICE: &str = "com.enaboapps.switchify.pc.account";
const KEYRING_USER: &str = "session";
pub const ACCOUNT_EVENT: &str = "account-changed";
/// Refresh this long before the access token expires.
const REFRESH_MARGIN: Duration = Duration::from_secs(60);
/// Pairing waits on these, so they are much shorter than the HTTP timeout.
const PAIRING_INTENT_TIMEOUT: Duration = Duration::from_secs(5);
const PAIRING_INTENT_RETRY_DELAY: Duration = Duration::from_secs(1);
const KEYCHAIN_UNAVAILABLE: &str =
    "This computer's keychain is unavailable. Unlock it, then try again.";

#[derive(Clone)]
struct Config {
    url: String,
    key: String,
}

impl Config {
    fn from_build() -> Option<Self> {
        Self::new(
            option_env!("SWITCHIFY_SUPABASE_URL").unwrap_or_default(),
            option_env!("SWITCHIFY_SUPABASE_PUBLISHABLE_KEY").unwrap_or_default(),
        )
    }

    fn new(url: &str, key: &str) -> Option<Self> {
        let url = url.trim().trim_end_matches('/').to_owned();
        let key = key.trim().to_owned();
        (url.starts_with("https://") && !key.is_empty()).then_some(Self { url, key })
    }
}

struct Response {
    status: u16,
    body: Value,
}

type ResponseFuture = Pin<Box<dyn Future<Output = Result<Response, String>> + Send>>;

trait Transport: Send + Sync {
    /// POSTs JSON with the publishable key. `bearer` is a user access token.
    fn post(
        &self,
        config: &Config,
        path: &str,
        bearer: Option<&str>,
        body: Value,
    ) -> ResponseFuture;
}

struct HttpTransport {
    client: reqwest::Client,
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl Transport for HttpTransport {
    fn post(
        &self,
        config: &Config,
        path: &str,
        bearer: Option<&str>,
        body: Value,
    ) -> ResponseFuture {
        let mut request = self
            .client
            .post(format!("{}{path}", config.url))
            .header("apikey", &config.key)
            .json(&body);
        // Publishable keys are not JWTs, so only user tokens go in Authorization.
        if let Some(token) = bearer {
            request = request.bearer_auth(token);
        }
        Box::pin(async move {
            // reqwest errors can include the URL but never headers or bodies.
            let response = request
                .send()
                .await
                .map_err(|_| "Could not reach Switchify. Check your connection.".to_string())?;
            let status = response.status().as_u16();
            let body = response.json::<Value>().await.unwrap_or(Value::Null);
            Ok(Response { status, body })
        })
    }
}

trait SessionStore: Send + Sync {
    fn load(&self) -> Result<Option<String>, String>;
    fn save(&self, value: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

struct KeyringSessionStore;

impl KeyringSessionStore {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|error| error.to_string())
    }
}

impl SessionStore for KeyringSessionStore {
    /// Only an unreachable keychain is an error (retried later). An entry
    /// that cannot be decoded or is ambiguous is removed and treated as
    /// signed out, so it can never lock the user out of signing in.
    fn load(&self) -> Result<Option<String>, String> {
        let entry = Self::entry()?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(
                error @ (keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_)),
            ) => Err(error.to_string()),
            Err(_) => {
                let _ = entry.delete_credential();
                Ok(None)
            }
        }
    }

    fn save(&self, value: &str) -> Result<(), String> {
        Self::entry()?
            .set_password(value)
            .map_err(|error| error.to_string())
    }

    fn delete(&self) -> Result<(), String> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

/// What survives a restart. Kept in the keychain as one JSON entry.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSession {
    user_id: String,
    email: String,
    refresh_token: String,
}

struct Session {
    stored: StoredSession,
    access: Option<(String, Instant)>,
}

#[derive(Default)]
struct Data {
    session: Option<Session>,
    pending_email: Option<String>,
    /// False while the keychain could not be read (e.g. locked at login);
    /// reading is retried on the next account call.
    loaded: bool,
}

/// Sanitized state for the webview. Never includes tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub available: bool,
    pub signed_in: bool,
    pub email: Option<String>,
    pub pending_email: Option<String>,
    /// The saved sign-in could not be read from the keychain yet.
    pub keychain_unavailable: bool,
}

/// A usable access token for the signed-in user.
#[derive(Clone)]
pub struct Authorized {
    pub user_id: String,
    pub access_token: String,
}

pub struct Account {
    config: Option<Config>,
    transport: Arc<dyn Transport>,
    store: Arc<dyn SessionStore>,
    // Async so a refresh holds the lock: refresh tokens rotate, and two
    // concurrent refreshes would revoke the session.
    data: tokio::sync::Mutex<Data>,
}

impl Account {
    pub fn install() -> Self {
        Self::new(
            Config::from_build(),
            Arc::new(HttpTransport::default()),
            Arc::new(KeyringSessionStore),
        )
    }

    fn new(
        config: Option<Config>,
        transport: Arc<dyn Transport>,
        store: Arc<dyn SessionStore>,
    ) -> Self {
        let account = Self {
            config,
            transport,
            store,
            data: tokio::sync::Mutex::new(Data::default()),
        };
        account.load(account.data.try_lock().as_deref_mut().expect("new lock"));
        account
    }

    /// Reads the saved session once. An unreadable keychain is retried later;
    /// an absent or corrupt entry counts as signed out (the next sign-in
    /// overwrites it).
    fn load(&self, data: &mut Data) {
        if data.loaded || self.config.is_none() {
            return;
        }
        if let Ok(value) = self.store.load() {
            data.session = value
                .and_then(|value| serde_json::from_str::<StoredSession>(&value).ok())
                .map(|stored| Session {
                    stored,
                    access: None,
                });
            data.loaded = true;
        }
    }

    /// `retry_keychain` re-reads a keychain that was unreadable. Only user
    /// actions retry, so background calls never trigger keychain prompts.
    async fn lock(&self, retry_keychain: bool) -> tokio::sync::MutexGuard<'_, Data> {
        let mut data = self.data.lock().await;
        if retry_keychain {
            self.load(&mut data);
        }
        data
    }

    /// The view without re-reading the keychain, for reporting state after an
    /// action (a second read could prompt again on a locked keychain).
    pub async fn current_view(&self) -> AccountView {
        let data = self.lock(false).await;
        self.view_of(&data)
    }

    pub async fn view(&self) -> AccountView {
        let data = self.lock(true).await;
        self.view_of(&data)
    }

    fn view_of(&self, data: &Data) -> AccountView {
        AccountView {
            available: self.config.is_some(),
            signed_in: data.session.is_some(),
            email: data.session.as_ref().map(|s| s.stored.email.clone()),
            pending_email: data.pending_email.clone(),
            keychain_unavailable: self.config.is_some() && !data.loaded,
        }
    }

    /// Forgets the session on this install. Memory is cleared first so the
    /// app never shows a session that is gone. If the keychain also cannot
    /// delete, the refresh token stays behind: it is dead after a server
    /// sign-out or account deletion, but if that call failed too (offline)
    /// the next launch can restore the session. Both failing is rare.
    fn forget(&self, data: &mut Data) {
        data.session = None;
        data.pending_email = None;
        let _ = self.store.delete();
    }

    /// The project URL and publishable key, for the database requests made by
    /// settings sync with an `Authorized` token.
    pub fn endpoint(&self) -> Option<(String, String)> {
        self.config
            .as_ref()
            .map(|config| (config.url.clone(), config.key.clone()))
    }

    fn config(&self) -> Result<&Config, String> {
        self.config
            .as_ref()
            .ok_or_else(|| "Accounts are unavailable in this build.".to_string())
    }

    pub async fn request_code(&self, email: &str) -> Result<AccountView, String> {
        let config = self.config()?;
        let email = normalize_email(email)?;
        let mut data = self.lock(true).await;
        if !data.loaded {
            return Err(KEYCHAIN_UNAVAILABLE.into());
        }
        if data.session.is_some() {
            return Err("Sign out before signing in with another email.".into());
        }
        let response = self
            .transport
            .post(
                config,
                "/auth/v1/otp",
                None,
                json!({ "email": email, "create_user": true }),
            )
            .await?;
        match response.status {
            200..=299 => {
                data.pending_email = Some(email);
                Ok(self.view_of(&data))
            }
            429 => Err("Too many codes requested. Wait a minute, then try again.".into()),
            _ => Err("Could not send a sign-in code. Try again.".into()),
        }
    }

    pub async fn verify_code(&self, code: &str) -> Result<AccountView, String> {
        let config = self.config()?;
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        if !(6..=10).contains(&code.len()) || !code.chars().all(|c| c.is_ascii_digit()) {
            return Err("Enter the code from the email.".into());
        }
        let mut data = self.lock(false).await;
        let email = data
            .pending_email
            .clone()
            .ok_or("Request a sign-in code first.")?;
        let response = self
            .transport
            .post(
                config,
                "/auth/v1/verify",
                None,
                json!({ "type": "email", "email": email, "token": code }),
            )
            .await?;
        match response.status {
            200..=299 => {}
            429 => return Err("Too many attempts. Wait a minute, then try again.".into()),
            400..=499 => return Err("That code is wrong or has expired.".into()),
            _ => return Err("Could not sign in. Try again.".into()),
        }
        let session = parse_session(&response.body, None)?;
        self.persist(&session.stored)?;
        data.session = Some(session);
        data.pending_email = None;
        Ok(self.view_of(&data))
    }

    pub async fn cancel_code(&self) -> AccountView {
        let mut data = self.lock(false).await;
        data.pending_email = None;
        self.view_of(&data)
    }

    /// Returns a fresh access token, refreshing it if needed. A rejected
    /// refresh token signs this install out.
    pub async fn authorize(&self) -> Result<Authorized, String> {
        let config = self.config()?;
        let mut data = self.lock(false).await;
        let session = data.session.as_ref().ok_or("Sign in first.")?;
        if let Some((token, expires)) = &session.access {
            if Instant::now() + REFRESH_MARGIN < *expires {
                return Ok(Authorized {
                    user_id: session.stored.user_id.clone(),
                    access_token: token.clone(),
                });
            }
        }
        let response = self
            .transport
            .post(
                config,
                "/auth/v1/token?grant_type=refresh_token",
                None,
                json!({ "refresh_token": session.stored.refresh_token }),
            )
            .await?;
        match response.status {
            200..=299 => {}
            400 | 401 | 403 => {
                self.forget(&mut data);
                return Err("You were signed out. Sign in again.".into());
            }
            _ => return Err("Could not reach Switchify. Try again later.".into()),
        }
        let previous = session.stored.clone();
        let refreshed = parse_session(&response.body, Some(&previous))?;
        // The server has already rotated the refresh token, so the new one
        // must be kept in memory even if the keychain write fails; reusing the
        // old one later would revoke the session. Only a restart before the
        // next successful save falls back to the old token.
        let _ = self.persist(&refreshed.stored);
        let authorized = Authorized {
            user_id: refreshed.stored.user_id.clone(),
            access_token: refreshed
                .access
                .as_ref()
                .map(|(token, _)| token.clone())
                .unwrap_or_default(),
        };
        data.session = Some(refreshed);
        Ok(authorized)
    }

    /// Ends the session on this install. The server call is best effort;
    /// local credentials are always removed.
    pub async fn sign_out(&self) -> Result<AccountView, String> {
        let config = self.config()?.clone();
        let token = self.authorize().await.ok().map(|a| a.access_token);
        if let Some(token) = token {
            let _ = self
                .transport
                .post(
                    &config,
                    "/auth/v1/logout?scope=local",
                    Some(&token),
                    json!({}),
                )
                .await;
        }
        let mut data = self.lock(false).await;
        self.forget(&mut data);
        Ok(self.view_of(&data))
    }

    /// Permanently deletes the account and its synced data on every device,
    /// the same way the Android app does.
    pub async fn delete_account(&self) -> Result<AccountView, String> {
        let config = self.config()?.clone();
        let authorized = self.authorize().await?;
        let response = self
            .transport
            .post(
                &config,
                "/rest/v1/rpc/delete_user_account",
                Some(&authorized.access_token),
                json!({}),
            )
            .await?;
        if !(200..=299).contains(&response.status) {
            return Err("Could not delete the account. Try again.".into());
        }
        let mut data = self.lock(false).await;
        self.forget(&mut data);
        Ok(self.view_of(&data))
    }

    /// Whether a phone signed into this account vouched for the pairing
    /// request by recording a matching intent. A match is consumed, so it
    /// approves once. Signed out, offline or any server error is `false`, which
    /// leaves the request for manual approval.
    pub async fn consume_pairing_intent(&self, intent: &PairingIntent) -> bool {
        self.consume_pairing_intent_after(intent, PAIRING_INTENT_RETRY_DELAY)
            .await
    }

    async fn consume_pairing_intent_after(
        &self,
        intent: &PairingIntent,
        retry_delay: Duration,
    ) -> bool {
        let Ok(config) = self.config().cloned() else {
            return false;
        };
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(retry_delay).await;
            }
            let Ok(authorized) = self.authorize().await else {
                return false;
            };
            let request = self.transport.post(
                &config,
                "/rest/v1/rpc/consume_pairing_intent",
                Some(&authorized.access_token),
                json!({
                    "p_desktop_id": intent.desktop_id,
                    "p_device_id": intent.device_id,
                    "p_nonce": intent.nonce,
                }),
            );
            match tokio::time::timeout(PAIRING_INTENT_TIMEOUT, request).await {
                Ok(Ok(response)) if (200..=299).contains(&response.status) => {
                    return response.body == Value::Bool(true);
                }
                // The server answered; retrying would not change a refusal or
                // a missing function.
                Ok(Ok(response)) if response.status < 500 => return false,
                _ => {}
            }
        }
        false
    }

    fn persist(&self, stored: &StoredSession) -> Result<(), String> {
        let value = serde_json::to_string(stored).map_err(|error| error.to_string())?;
        self.store
            .save(&value)
            .map_err(|_| "Could not save the sign-in to this computer's keychain.".to_string())
    }
}

fn normalize_email(email: &str) -> Result<String, String> {
    let email = email.trim();
    let valid = email.len() <= 254
        && !email.chars().any(char::is_whitespace)
        && email.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && !domain.contains('@')
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        });
    if valid {
        Ok(email.to_owned())
    } else {
        Err("Enter a valid email address.".into())
    }
}

/// Reads a GoTrue session response. A refresh response may omit the user;
/// then the previous identity is kept.
fn parse_session(body: &Value, previous: Option<&StoredSession>) -> Result<Session, String> {
    let invalid = || "Switchify sent an unexpected response. Try again.".to_string();
    let text = |value: &Value| value.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let access_token = text(&body["access_token"]).ok_or_else(invalid)?;
    let refresh_token = text(&body["refresh_token"]).ok_or_else(invalid)?;
    let expires_in = body["expires_in"].as_u64().ok_or_else(invalid)?;
    let user_id = text(&body["user"]["id"])
        .or_else(|| previous.map(|p| p.user_id.clone()))
        .ok_or_else(invalid)?;
    let email = text(&body["user"]["email"])
        .or_else(|| previous.map(|p| p.email.clone()))
        .ok_or_else(invalid)?;
    if previous.is_some_and(|p| p.user_id != user_id) {
        return Err(invalid());
    }
    Ok(Session {
        stored: StoredSession {
            user_id,
            email,
            refresh_token,
        },
        access: Some((
            access_token,
            Instant::now() + Duration::from_secs(expires_in),
        )),
    })
}

pub fn emit(app: &AppHandle, view: &AccountView) {
    let _ = app.emit(ACCOUNT_EVENT, view);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeTransport {
        requests: Mutex<Vec<(String, Option<String>, Value)>>,
        responses: Mutex<VecDeque<Result<Response, String>>>,
    }

    impl FakeTransport {
        fn respond(&self, status: u16, body: Value) {
            self.responses
                .lock()
                .unwrap()
                .push_back(Ok(Response { status, body }));
        }
        fn fail(&self) {
            self.responses
                .lock()
                .unwrap()
                .push_back(Err("offline".into()));
        }
        fn requests(&self) -> Vec<(String, Option<String>, Value)> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl Transport for FakeTransport {
        fn post(
            &self,
            _config: &Config,
            path: &str,
            bearer: Option<&str>,
            body: Value,
        ) -> ResponseFuture {
            self.requests
                .lock()
                .unwrap()
                .push((path.into(), bearer.map(str::to_owned), body));
            let response = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected request");
            Box::pin(async move { response })
        }
    }

    #[derive(Default)]
    struct MemoryStore {
        value: Mutex<Option<String>>,
        fail_load: AtomicBool,
        fail_save: AtomicBool,
        fail_delete: AtomicBool,
    }

    impl MemoryStore {
        fn stored(&self) -> Option<String> {
            self.value.lock().unwrap().clone()
        }
    }

    impl SessionStore for MemoryStore {
        fn load(&self) -> Result<Option<String>, String> {
            if self.fail_load.load(Ordering::SeqCst) {
                return Err("locked".into());
            }
            Ok(self.stored())
        }
        fn save(&self, value: &str) -> Result<(), String> {
            if self.fail_save.load(Ordering::SeqCst) {
                return Err("locked".into());
            }
            *self.value.lock().unwrap() = Some(value.into());
            Ok(())
        }
        fn delete(&self) -> Result<(), String> {
            if self.fail_delete.load(Ordering::SeqCst) {
                return Err("locked".into());
            }
            *self.value.lock().unwrap() = None;
            Ok(())
        }
    }

    fn config() -> Option<Config> {
        Config::new("https://example.supabase.co/", "publishable")
    }

    fn account(store: Arc<MemoryStore>) -> (Account, Arc<FakeTransport>) {
        let transport = Arc::new(FakeTransport::default());
        (Account::new(config(), transport.clone(), store), transport)
    }

    fn session_body(access: &str, refresh: &str, expires_in: u64) -> Value {
        json!({
            "access_token": access,
            "refresh_token": refresh,
            "expires_in": expires_in,
            "user": { "id": "user-1", "email": "me@example.com" }
        })
    }

    async fn signed_in(store: Arc<MemoryStore>) -> (Account, Arc<FakeTransport>) {
        let (account, transport) = account(store);
        transport.respond(200, json!({}));
        account.request_code("me@example.com").await.unwrap();
        transport.respond(200, session_body("access-1", "refresh-1", 3600));
        account.verify_code("123456").await.unwrap();
        (account, transport)
    }

    #[test]
    fn config_requires_https_and_a_key() {
        assert!(Config::new("http://example.supabase.co", "key").is_none());
        assert!(Config::new("https://example.supabase.co", " ").is_none());
        assert_eq!(
            Config::new(" https://example.supabase.co/ ", "key")
                .unwrap()
                .url,
            "https://example.supabase.co"
        );
    }

    #[test]
    fn keyring_identity_is_stable() {
        assert_eq!(KEYRING_SERVICE, "com.enaboapps.switchify.pc.account");
        assert_eq!(KEYRING_USER, "session");
    }

    #[tokio::test]
    async fn unavailable_without_build_config() {
        let account = Account::new(
            None,
            Arc::new(FakeTransport::default()),
            Arc::new(MemoryStore::default()),
        );
        assert!(!account.view().await.available);
        assert!(account.request_code("me@example.com").await.is_err());
    }

    #[tokio::test]
    async fn rejects_invalid_email_and_code_without_requests() {
        let (account, transport) = account(Arc::default());
        for email in [
            "",
            "me",
            "me@",
            "@example.com",
            "me@example",
            "m e@example.com",
        ] {
            assert!(account.request_code(email).await.is_err(), "{email}");
        }
        assert!(
            account.verify_code("123456").await.is_err(),
            "no pending email"
        );
        assert!(transport.requests().is_empty());
        transport.respond(200, json!({}));
        account.request_code("me@example.com").await.unwrap();
        for code in ["12345", "abcdef", "12345678901"] {
            assert!(account.verify_code(code).await.is_err(), "{code}");
        }
        assert_eq!(transport.requests().len(), 1);
    }

    #[tokio::test]
    async fn sign_in_with_code_stores_only_the_refresh_session() {
        let store = Arc::new(MemoryStore::default());
        let (account, transport) = account(store.clone());
        transport.respond(200, json!({}));
        let view = account.request_code(" me@example.com ").await.unwrap();
        assert_eq!(view.pending_email.as_deref(), Some("me@example.com"));
        transport.respond(200, session_body("access-1", "refresh-1", 3600));
        let view = account.verify_code("123 456").await.unwrap();
        assert_eq!(
            view,
            AccountView {
                available: true,
                signed_in: true,
                email: Some("me@example.com".into()),
                pending_email: None,
                keychain_unavailable: false,
            }
        );
        let requests = transport.requests();
        assert_eq!(requests[0].0, "/auth/v1/otp");
        assert_eq!(
            requests[0].2,
            json!({"email": "me@example.com", "create_user": true})
        );
        assert_eq!(requests[1].0, "/auth/v1/verify");
        assert_eq!(
            requests[1].2,
            json!({"type": "email", "email": "me@example.com", "token": "123456"})
        );
        let stored = store.value.lock().unwrap().clone().unwrap();
        assert!(stored.contains("refresh-1") && !stored.contains("access-1"));
        let view_json = serde_json::to_string(&view).unwrap();
        assert!(!view_json.contains("refresh") && !view_json.contains("access-1"));
    }

    #[tokio::test]
    async fn wrong_code_keeps_the_pending_email() {
        let (account, transport) = account(Arc::default());
        transport.respond(200, json!({}));
        account.request_code("me@example.com").await.unwrap();
        transport.respond(403, json!({"msg": "Token has expired or is invalid"}));
        assert_eq!(
            account.verify_code("000000").await.unwrap_err(),
            "That code is wrong or has expired."
        );
        let view = account.view().await;
        assert!(!view.signed_in);
        assert_eq!(view.pending_email.as_deref(), Some("me@example.com"));
    }

    #[tokio::test]
    async fn rate_limits_and_network_errors_are_reported() {
        let (account, transport) = account(Arc::default());
        transport.respond(429, json!({}));
        assert!(account
            .request_code("me@example.com")
            .await
            .unwrap_err()
            .contains("Wait a minute"));
        transport.fail();
        assert!(account.request_code("me@example.com").await.is_err());
        assert!(account.view().await.pending_email.is_none());
    }

    #[tokio::test]
    async fn keychain_failure_does_not_sign_in() {
        let store = Arc::new(MemoryStore::default());
        store.fail_save.store(true, Ordering::SeqCst);
        let (account, transport) = account(store);
        transport.respond(200, json!({}));
        account.request_code("me@example.com").await.unwrap();
        transport.respond(200, session_body("a", "r", 3600));
        assert!(account.verify_code("123456").await.is_err());
        assert!(!account.view().await.signed_in);
    }

    #[tokio::test]
    async fn session_survives_restart_and_refreshes() {
        let store = Arc::new(MemoryStore::default());
        signed_in(store.clone()).await;
        let (restarted, transport) = account(store.clone());
        assert!(restarted.view().await.signed_in);
        transport.respond(
            200,
            json!({"access_token": "access-2", "refresh_token": "refresh-2", "expires_in": 3600}),
        );
        let authorized = restarted.authorize().await.unwrap();
        assert_eq!(authorized.access_token, "access-2");
        assert_eq!(authorized.user_id, "user-1");
        let requests = transport.requests();
        assert_eq!(requests[0].0, "/auth/v1/token?grant_type=refresh_token");
        assert_eq!(requests[0].2, json!({"refresh_token": "refresh-1"}));
        assert!(store
            .value
            .lock()
            .unwrap()
            .clone()
            .unwrap()
            .contains("refresh-2"));
        // Cached until close to expiry: no second request.
        restarted.authorize().await.unwrap();
        assert_eq!(transport.requests().len(), 1);
    }

    #[tokio::test]
    async fn expiring_token_is_refreshed() {
        let (account, transport) = account(Arc::default());
        transport.respond(200, json!({}));
        account.request_code("me@example.com").await.unwrap();
        transport.respond(200, session_body("access-1", "refresh-1", 30));
        account.verify_code("123456").await.unwrap();
        transport.respond(200, session_body("access-2", "refresh-2", 3600));
        assert_eq!(account.authorize().await.unwrap().access_token, "access-2");
    }

    #[tokio::test]
    async fn rejected_refresh_signs_out_but_outage_does_not() {
        let store = Arc::new(MemoryStore::default());
        signed_in(store.clone()).await;
        let (account, transport) = account(store.clone());
        transport.fail();
        assert!(account.authorize().await.is_err());
        transport.respond(503, json!({}));
        assert!(account.authorize().await.is_err());
        assert!(account.view().await.signed_in, "outages keep the session");
        transport.respond(400, json!({"error": "invalid_grant"}));
        assert!(account.authorize().await.is_err());
        assert!(!account.view().await.signed_in);
        assert!(store.value.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn refresh_for_a_different_user_is_rejected() {
        let store = Arc::new(MemoryStore::default());
        signed_in(store.clone()).await;
        let (account, transport) = account(store.clone());
        let saved = store.stored();
        let mut body = session_body("a", "r", 3600);
        body["user"]["id"] = json!("someone-else");
        transport.respond(200, body);
        assert!(account.authorize().await.is_err());
        assert_eq!(store.stored(), saved, "mismatched session is not saved");
        assert_eq!(
            account.view().await.email.as_deref(),
            Some("me@example.com")
        );
    }

    #[tokio::test]
    async fn rotated_refresh_token_is_kept_when_the_keychain_save_fails() {
        let store = Arc::new(MemoryStore::default());
        let (account, transport) = signed_in(store.clone()).await;
        let saved = store.stored();
        store.fail_save.store(true, Ordering::SeqCst);
        // Force a refresh by expiring the cached access token.
        account.data.lock().await.session.as_mut().unwrap().access = None;
        transport.respond(200, session_body("access-2", "refresh-2", 3600));
        assert_eq!(account.authorize().await.unwrap().access_token, "access-2");
        assert_eq!(store.stored(), saved, "keychain keeps the last saved copy");
        // The next refresh must use the rotated token, not the revoked one.
        account.data.lock().await.session.as_mut().unwrap().access = None;
        transport.respond(200, session_body("access-3", "refresh-3", 3600));
        account.authorize().await.unwrap();
        assert_eq!(
            transport.requests().last().unwrap().2,
            json!({"refresh_token": "refresh-2"})
        );
    }

    #[tokio::test]
    async fn locked_keychain_at_startup_is_retried() {
        let store = Arc::new(MemoryStore::default());
        signed_in(store.clone()).await;
        store.fail_load.store(true, Ordering::SeqCst);
        let (account, transport) = account(store.clone());
        let view = account.view().await;
        assert!(!view.signed_in && view.keychain_unavailable);
        assert!(account.request_code("me@example.com").await.is_err());
        assert!(
            transport.requests().is_empty(),
            "no sign-in over a hidden session"
        );
        store.fail_load.store(false, Ordering::SeqCst);
        // Background calls never re-read the keychain (no prompts)...
        assert!(account.authorize().await.is_err());
        assert!(transport.requests().is_empty());
        // ...but opening the Account page does.
        let view = account.view().await;
        assert!(view.signed_in && !view.keychain_unavailable);
    }

    #[tokio::test]
    async fn corrupt_keychain_entry_counts_as_signed_out() {
        let store = Arc::new(MemoryStore::default());
        *store.value.lock().unwrap() = Some("not json".into());
        let (account, _) = account(store);
        let view = account.view().await;
        assert!(!view.signed_in && !view.keychain_unavailable);
    }

    #[tokio::test]
    async fn keychain_delete_failure_still_signs_out_and_deletes() {
        let store = Arc::new(MemoryStore::default());
        let (account, transport) = signed_in(store.clone()).await;
        store.fail_delete.store(true, Ordering::SeqCst);
        transport.respond(204, Value::Null);
        assert!(!account.delete_account().await.unwrap().signed_in);
        let store = Arc::new(MemoryStore::default());
        let (account, transport) = signed_in(store.clone()).await;
        store.fail_delete.store(true, Ordering::SeqCst);
        transport.respond(200, json!({}));
        let view = account.sign_out().await.unwrap();
        assert!(!view.signed_in && view.email.is_none());
        assert!(account.authorize().await.is_err(), "memory session is gone");
    }

    #[tokio::test]
    async fn sign_out_clears_credentials_even_offline() {
        let store = Arc::new(MemoryStore::default());
        let (account, transport) = signed_in(store.clone()).await;
        transport.fail();
        let view = account.sign_out().await.unwrap();
        assert!(!view.signed_in && view.email.is_none());
        assert!(store.value.lock().unwrap().is_none());
        let logout = transport.requests().pop().unwrap();
        assert_eq!(logout.0, "/auth/v1/logout?scope=local");
        assert_eq!(logout.1.as_deref(), Some("access-1"));
    }

    #[tokio::test]
    async fn delete_account_calls_the_shared_rpc_then_signs_out() {
        let store = Arc::new(MemoryStore::default());
        let (account, transport) = signed_in(store.clone()).await;
        transport.respond(500, json!({}));
        assert!(account.delete_account().await.is_err());
        assert!(
            account.view().await.signed_in,
            "failed delete keeps the session"
        );
        transport.respond(204, Value::Null);
        assert!(!account.delete_account().await.unwrap().signed_in);
        assert!(store.value.lock().unwrap().is_none());
        let rpc = transport.requests().pop().unwrap();
        assert_eq!(rpc.0, "/rest/v1/rpc/delete_user_account");
        assert_eq!(rpc.1.as_deref(), Some("access-1"));
    }

    fn intent() -> PairingIntent {
        PairingIntent {
            desktop_id: "desktop-1".into(),
            device_id: "android-1".into(),
            nonce: "nonce-1".into(),
        }
    }

    #[tokio::test]
    async fn pairing_intent_is_consumed_with_the_user_session() {
        let (account, transport) = signed_in(Arc::default()).await;
        transport.respond(200, json!(true));
        assert!(
            account
                .consume_pairing_intent_after(&intent(), Duration::ZERO)
                .await
        );
        let rpc = transport.requests().pop().unwrap();
        assert_eq!(rpc.0, "/rest/v1/rpc/consume_pairing_intent");
        assert_eq!(rpc.1.as_deref(), Some("access-1"));
        assert_eq!(
            rpc.2,
            json!({ "p_desktop_id": "desktop-1", "p_device_id": "android-1", "p_nonce": "nonce-1" })
        );
    }

    #[tokio::test]
    async fn pairing_intent_without_a_match_or_function_is_not_retried() {
        let (account, transport) = signed_in(Arc::default()).await;
        let before = transport.requests().len();
        transport.respond(200, json!(false));
        assert!(
            !account
                .consume_pairing_intent_after(&intent(), Duration::ZERO)
                .await
        );
        transport.respond(404, json!({ "code": "PGRST202" }));
        assert!(
            !account
                .consume_pairing_intent_after(&intent(), Duration::ZERO)
                .await
        );
        assert_eq!(transport.requests().len(), before + 2);
    }

    #[tokio::test]
    async fn pairing_intent_retries_once_after_a_network_or_server_error() {
        let (account, transport) = signed_in(Arc::default()).await;
        transport.fail();
        transport.respond(200, json!(true));
        assert!(
            account
                .consume_pairing_intent_after(&intent(), Duration::ZERO)
                .await
        );
        let before = transport.requests().len();
        transport.respond(503, json!({}));
        transport.fail();
        assert!(
            !account
                .consume_pairing_intent_after(&intent(), Duration::ZERO)
                .await
        );
        assert_eq!(transport.requests().len(), before + 2);
    }

    #[tokio::test]
    async fn signed_out_install_never_checks_pairing_intents() {
        let transport = Arc::new(FakeTransport::default());
        let account = Account::new(
            config(),
            transport.clone(),
            Arc::new(MemoryStore::default()),
        );
        assert!(
            !account
                .consume_pairing_intent_after(&intent(), Duration::ZERO)
                .await
        );
        assert!(transport.requests().is_empty());
        let unconfigured = Account::new(None, transport.clone(), Arc::new(MemoryStore::default()));
        assert!(
            !unconfigured
                .consume_pairing_intent_after(&intent(), Duration::ZERO)
                .await
        );
        assert!(transport.requests().is_empty());
    }

    #[tokio::test]
    async fn signed_in_install_cannot_request_another_code() {
        let (account, transport) = signed_in(Arc::default()).await;
        assert!(account.request_code("other@example.com").await.is_err());
        assert_eq!(transport.requests().len(), 2);
    }

    /// End-to-end against a local `supabase start` stack (switchify-supabase),
    /// reading the code from its Mailpit inbox. Run with:
    /// SWITCHIFY_LOCAL_SUPABASE_URL=http://127.0.0.1:54321
    /// SWITCHIFY_LOCAL_SUPABASE_KEY=<local publishable key>
    /// cargo test account::tests::local_stack -- --ignored
    #[tokio::test]
    #[ignore = "needs a local Supabase stack"]
    async fn local_stack_sign_in_refresh_and_delete() {
        let url = std::env::var("SWITCHIFY_LOCAL_SUPABASE_URL").unwrap();
        let key = std::env::var("SWITCHIFY_LOCAL_SUPABASE_KEY").unwrap();
        let mailpit = std::env::var("SWITCHIFY_LOCAL_MAILPIT_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:54324".into());
        // Local stacks are plain HTTP, which Config::new rightly refuses.
        let config = Config { url, key };
        let store = Arc::new(MemoryStore::default());
        let account = Account::new(
            Some(config.clone()),
            Arc::new(HttpTransport::default()),
            store.clone(),
        );
        let email = format!("it-{}@example.test", uuid::Uuid::new_v4());
        account.request_code(&email).await.unwrap();

        let client = reqwest::Client::new();
        let mut code = None;
        for _ in 0..20 {
            let search: Value = client
                .get(format!("{mailpit}/api/v1/search?query=to:{email}"))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            if let Some(id) = search["messages"][0]["ID"].as_str() {
                let message: Value = client
                    .get(format!("{mailpit}/api/v1/message/{id}"))
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                let text = message["Text"].as_str().unwrap_or_default();
                // The magic-link URL's token can contain digit runs too, so
                // read only what follows "enter the code:".
                code = text.rsplit_once("code:").map(|(_, rest)| {
                    rest.chars()
                        .filter(char::is_ascii_digit)
                        .collect::<String>()
                });
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        let view = account
            .verify_code(&code.expect("no code email"))
            .await
            .unwrap();
        assert!(view.signed_in);
        assert_eq!(view.email.as_deref(), Some(email.as_str()));

        // A restart refreshes from the stored refresh token.
        let restarted = Account::new(
            Some(config.clone()),
            Arc::new(HttpTransport::default()),
            store.clone(),
        );
        let authorized = restarted.authorize().await.unwrap();
        assert!(!authorized.user_id.is_empty());

        // A phone on the same account records an intent; it approves once.
        let pairing = PairingIntent {
            desktop_id: "desktop-it".into(),
            device_id: "android-it".into(),
            nonce: uuid::Uuid::new_v4().to_string(),
        };
        assert!(!restarted.consume_pairing_intent(&pairing).await);
        let created = client
            .post(format!("{}/rest/v1/rpc/create_pairing_intent", config.url))
            .header("apikey", &config.key)
            .bearer_auth(&authorized.access_token)
            .json(&json!({
                "p_desktop_id": pairing.desktop_id,
                "p_device_id": pairing.device_id,
                "p_nonce": pairing.nonce,
            }))
            .send()
            .await
            .unwrap();
        assert!(created.status().is_success());
        assert!(restarted.consume_pairing_intent(&pairing).await);
        assert!(!restarted.consume_pairing_intent(&pairing).await);

        // The access token works for the shared delete RPC.
        assert!(!restarted.delete_account().await.unwrap().signed_in);
        assert!(store.value.lock().unwrap().is_none());
    }
}
