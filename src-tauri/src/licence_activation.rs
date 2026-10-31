//! Network and installation identity adapters. Never expose credentials to the
//! webview or logs. Tests use injected transports and in-memory identities.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{future::Future, pin::Pin, time::Duration};

const SERVICE: &str = "com.enaboapps.switchify.pc.licence";
pub const NETWORK_ERROR: &str = "Could not check your licence. Connect to the internet and try again. Your existing offline allowance is unchanged.";
pub const INVALID_REQUEST_ERROR: &str = "Could not verify this licence request. Check your key and your computer's date and time, or contact support. Your existing offline allowance is unchanged.";
pub const STORAGE_ERROR: &str = "Your installation key is unavailable. Unlock your keychain and try again. Your existing offline allowance is unchanged.";

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Installation {
    pub id: String,
    pub public_key: String,
}
pub struct Identity {
    pub installation: Installation,
    key: SigningKey,
}
impl Identity {
    // A separate domain prevents a signed HTTP request being reused as a cache seal.
    pub fn seal_cache(&self, bytes: &[u8]) -> String {
        let message = [b"Switchify PC local cache v1\0".as_slice(), bytes].concat();
        URL_SAFE_NO_PAD.encode(self.key.sign(&message).to_bytes())
    }
    pub fn verify_cache(&self, bytes: &[u8], signature: &str) -> bool {
        let Ok(bytes_signature) = URL_SAFE_NO_PAD.decode(signature) else {
            return false;
        };
        let Ok(signature) = Signature::from_slice(&bytes_signature) else {
            return false;
        };
        let message = [b"Switchify PC local cache v1\0".as_slice(), bytes].concat();
        self.key
            .verifying_key()
            .verify_strict(&message, &signature)
            .is_ok()
    }
    fn from_seed(id: String, seed: [u8; 32]) -> Self {
        let key = SigningKey::from_bytes(&seed);
        Self {
            installation: Installation {
                id,
                public_key: URL_SAFE_NO_PAD.encode(key.verifying_key().as_bytes()),
            },
            key,
        }
    }
    pub fn sign(&self, request: Value) -> Value {
        // serde_json objects use sorted keys; only fixed integer/string/null/
        // boolean fields occur here, matching backend canonical JSON exactly.
        let bytes = serde_json::to_vec(&request).expect("fixed activation request serializes");
        json!({"request":request,"signature":URL_SAFE_NO_PAD.encode(self.key.sign(&bytes).to_bytes())})
    }
}
pub trait IdentityStore: Send + Sync {
    fn load_or_create(&self) -> Result<Identity, String>;
}
pub struct OsIdentityStore;
pub trait MirrorStore: Send + Sync {
    fn read(&self) -> Result<Option<String>, String>;
    fn write(&self, text: &str) -> Result<(), String>;
}
pub struct OsMirrorStore;
impl MirrorStore for OsMirrorStore {
    fn read(&self) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(SERVICE, "record").map_err(|_| STORAGE_ERROR)?;
        match entry.get_password() {
            Ok(text) => Ok(Some(text)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(STORAGE_ERROR.into()),
        }
    }
    fn write(&self, text: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, "record")
            .map_err(|_| STORAGE_ERROR)?
            .set_password(text)
            .map_err(|_| STORAGE_ERROR.into())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredIdentity {
    id: String,
    seed: String,
}
impl IdentityStore for OsIdentityStore {
    fn load_or_create(&self) -> Result<Identity, String> {
        let entry = keyring::Entry::new(SERVICE, "installation").map_err(|_| STORAGE_ERROR)?;
        match entry.get_password() {
            Ok(text) => {
                if text.len() > 1024 {
                    return Err(STORAGE_ERROR.into());
                }
                let stored: StoredIdentity =
                    serde_json::from_str(&text).map_err(|_| STORAGE_ERROR)?;
                let id = uuid::Uuid::parse_str(&stored.id).map_err(|_| STORAGE_ERROR)?;
                if id.to_string() != stored.id {
                    return Err(STORAGE_ERROR.into());
                }
                let seed: [u8; 32] = URL_SAFE_NO_PAD
                    .decode(stored.seed)
                    .map_err(|_| STORAGE_ERROR)?
                    .try_into()
                    .map_err(|_| STORAGE_ERROR)?;
                Ok(Identity::from_seed(stored.id, seed))
            }
            Err(keyring::Error::NoEntry) => {
                use rand::Rng as _;
                let mut seed = [0u8; 32];
                rand::rng().fill_bytes(&mut seed);
                let id = uuid::Uuid::new_v4().to_string();
                let stored = StoredIdentity {
                    id: id.clone(),
                    seed: URL_SAFE_NO_PAD.encode(seed),
                };
                entry
                    .set_password(&serde_json::to_string(&stored).map_err(|_| STORAGE_ERROR)?)
                    .map_err(|_| STORAGE_ERROR)?;
                Ok(Identity::from_seed(id, seed))
            }
            Err(_) => Err(STORAGE_ERROR.into()),
        }
    }
}
#[derive(Clone)]
pub struct Config {
    url: String,
    key: String,
}
impl Config {
    #[cfg(test)]
    pub fn test_config() -> Self {
        Self::new("https://example.supabase.co", "public").unwrap()
    }
    pub fn from_build() -> Option<Self> {
        Self::new(
            option_env!("SWITCHIFY_SUPABASE_URL").unwrap_or_default(),
            option_env!("SWITCHIFY_SUPABASE_PUBLISHABLE_KEY").unwrap_or_default(),
        )
    }
    fn new(url: &str, key: &str) -> Option<Self> {
        let parsed = reqwest::Url::parse(url.trim()).ok()?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || key.trim().is_empty()
        {
            return None;
        }
        Some(Self {
            url: format!(
                "{}/functions/v1/pc-licence-activation",
                url.trim().trim_end_matches('/')
            ),
            key: key.trim().into(),
        })
    }
}
pub type ResponseFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;
pub trait Transport: Send + Sync {
    fn post(&self, config: &Config, body: Value) -> ResponseFuture;
}
pub struct HttpTransport;
fn decode_response(status: reqwest::StatusCode, bytes: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| NETWORK_ERROR)?;
    if status == reqwest::StatusCode::BAD_REQUEST {
        // A narrowly classified unsigned error gives guidance, never authority
        // to change an entitlement or accept a proof in an error response.
        return if value.as_object().is_some_and(|object| {
            object.len() == 1 && object.get("outcome").is_some_and(|v| v == "invalid")
        }) {
            Ok(value)
        } else {
            Err(NETWORK_ERROR.into())
        };
    }
    if !status.is_success() && status.as_u16() != 409 && status.as_u16() != 429 {
        return Err(NETWORK_ERROR.into());
    }
    Ok(value)
}
impl Transport for HttpTransport {
    fn post(&self, config: &Config, body: Value) -> ResponseFuture {
        let config = config.clone();
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| NETWORK_ERROR)?;
            let mut response = client
                .post(config.url)
                .header("apikey", config.key)
                .json(&body)
                .send()
                .await
                .map_err(|_| NETWORK_ERROR)?;
            let status = response.status();
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| NETWORK_ERROR)? {
                if bytes.len() + chunk.len() > 8192 {
                    return Err(NETWORK_ERROR.into());
                }
                bytes.extend_from_slice(&chunk);
            }
            decode_response(status, &bytes)
        })
    }
}
pub fn request(
    identity: &Identity,
    action: &str,
    licence_id: &str,
    purchase_key: Option<&str>,
    takeover: bool,
    timestamp: i64,
    nonce: &str,
) -> Value {
    identity.sign(json!({"version":1,"action":action,"licence_id":licence_id,"installation_id":identity.installation.id,
        "public_key":identity.installation.public_key,"timestamp":timestamp,"nonce":nonce,"takeover":takeover,"purchase_key":purchase_key}))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    #[test]
    fn only_the_exact_bad_request_outcome_gets_invalid_request_guidance() {
        use reqwest::StatusCode;
        let value = decode_response(StatusCode::BAD_REQUEST, br#"{"outcome":"invalid"}"#).unwrap();
        assert_eq!(value["outcome"], "invalid");
        for body in [
            b"not json".as_slice(),
            br#"{"outcome":"invalid","proof":"untrusted"}"#,
            br#"{"proof":"untrusted"}"#,
            br#"{"outcome":"revoked"}"#,
            br#"{"outcome":"unknown"}"#,
        ] {
            assert!(decode_response(StatusCode::BAD_REQUEST, body).is_err());
        }
        for status in [
            StatusCode::UNAUTHORIZED,
            StatusCode::NOT_FOUND,
            StatusCode::INTERNAL_SERVER_ERROR,
        ] {
            assert!(decode_response(status, br#"{"outcome":"invalid"}"#).is_err());
        }
        assert_eq!(
            decode_response(StatusCode::CONFLICT, br#"{"outcome":"seat_taken"}"#).unwrap()
                ["outcome"],
            "seat_taken"
        );
        assert_eq!(
            decode_response(
                StatusCode::TOO_MANY_REQUESTS,
                br#"{"outcome":"rate_limited"}"#
            )
            .unwrap()["outcome"],
            "rate_limited"
        );
    }
    pub struct FakeIdentity;
    pub struct OtherIdentity;
    impl IdentityStore for OtherIdentity {
        fn load_or_create(&self) -> Result<Identity, String> {
            Ok(Identity::from_seed(
                "6af44000-17b7-4d87-ad1d-32e9a31b331c".into(),
                [10; 32],
            ))
        }
    }
    #[test]
    fn local_cache_seals_bind_the_os_key_and_cannot_be_http_signatures() {
        let identity = FakeIdentity.load_or_create().unwrap();
        let bytes = br#"{"revision":1}"#;
        let seal = identity.seal_cache(bytes);
        assert!(identity.verify_cache(bytes, &seal));
        assert!(!identity.verify_cache(br#"{"revision":2}"#, &seal));
        assert!(!OtherIdentity
            .load_or_create()
            .unwrap()
            .verify_cache(bytes, &seal));
        let signed = identity.sign(serde_json::json!({"revision":1}));
        assert!(!identity.verify_cache(bytes, signed["signature"].as_str().unwrap()));
    }
    #[derive(Default)]
    pub struct MemoryMirror(std::sync::Mutex<Option<String>>);
    impl MirrorStore for MemoryMirror {
        fn read(&self) -> Result<Option<String>, String> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn write(&self, text: &str) -> Result<(), String> {
            *self.0.lock().unwrap() = Some(text.into());
            Ok(())
        }
    }
    impl IdentityStore for FakeIdentity {
        fn load_or_create(&self) -> Result<Identity, String> {
            Ok(Identity::from_seed(
                "6af44000-17b7-4d87-ad1d-32e9a31b331b".into(),
                [9; 32],
            ))
        }
    }
    #[test]
    fn request_matches_public_backend_vector_without_logging_credentials() {
        let vector: Value = serde_json::from_str(include_str!(
            "../../tools/licence-keygen/activation-test-vector.json"
        ))
        .unwrap();
        let identity = FakeIdentity.load_or_create().unwrap();
        let body = request(
            &identity,
            "renew",
            vector["request"]["licence_id"].as_str().unwrap(),
            None,
            false,
            1000,
            vector["request"]["nonce"].as_str().unwrap(),
        );
        assert_eq!(body["request"], vector["request"]);
        assert_eq!(body["signature"], vector["request_signature"]);
    }
    #[test]
    fn config_rejects_insecure_urls_and_credentials_in_urls() {
        assert!(Config::new("https://example.supabase.co", "public").is_some());
        for url in [
            "http://example.com",
            "https://user:secret@example.com",
            "https://example.com/path",
            "https://example.com/?key=value",
        ] {
            assert!(Config::new(url, "public").is_none());
        }
    }
}
