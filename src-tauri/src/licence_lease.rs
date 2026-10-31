//! Signed activation replies. Purchase credentials never belong in this trust
//! set. Cached leases verify without an unlocked keychain or a network request.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;

const MAX_BYTES: usize = 4096;
const MAX_EPOCH: i64 = 253_402_300_799;
pub const OFFLINE_SECONDS: i64 = 30 * 86400;

#[derive(Clone, Default)]
pub struct LeaseKeys(Vec<(String, [u8; 32])>);
impl LeaseKeys {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn new(
        leases: Vec<(String, [u8; 32])>,
        purchases: &[(String, [u8; 32])],
    ) -> Result<Self, Error> {
        if leases.iter().any(|(id, key)| {
            !crate::licence_key::valid_key_id(id)
                || purchases
                    .iter()
                    .any(|(purchase_id, purchase_key)| id == purchase_id || key == purchase_key)
        }) {
            return Err(Error::Invalid);
        }
        Ok(Self(leases))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Expired,
}

/// Expected identity is from local installation state, never from the reply.
pub struct Binding<'a> {
    pub licence_id: &'a str,
    pub installation_id: &'a str,
    pub installation_key: &'a str,
}
pub struct RequestContext<'a> {
    pub binding: Binding<'a>,
    pub nonce: &'a str,
    pub action: &'a str,
    pub requested_at: i64,
    pub min_generation: u64,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    pub version: u8,
    pub product: String,
    pub licence_id: String,
    pub installation_id: String,
    pub installation_key: String,
    pub generation: u64,
    pub issued_at: i64,
    pub request_nonce: String,
    pub action: String,
    pub lease_id: String,
    pub expires_at: i64,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub version: u8,
    pub product: String,
    pub licence_id: String,
    pub installation_id: String,
    pub installation_key: String,
    pub generation: u64,
    pub issued_at: i64,
    pub request_nonce: String,
    pub action: String,
    pub status: String,
}
pub enum Reply {
    Lease(Lease),
    Status(Status),
}
fn uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}
fn epoch(value: i64) -> bool {
    (0..=MAX_EPOCH).contains(&value)
}
fn action(value: &str) -> bool {
    matches!(value, "activate" | "renew" | "deactivate")
}
fn verified_payload(value: &str, prefix: &str, keys: &LeaseKeys) -> Result<Vec<u8>, Error> {
    if value.len() > MAX_BYTES {
        return Err(Error::Invalid);
    }
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 4 || parts[0] != prefix {
        return Err(Error::Invalid);
    }
    let (_, public) = keys
        .0
        .iter()
        .find(|(id, _)| id == parts[1])
        .ok_or(Error::Invalid)?;
    let public = VerifyingKey::from_bytes(public).map_err(|_| Error::Invalid)?;
    let signature = URL_SAFE_NO_PAD
        .decode(parts[3])
        .map_err(|_| Error::Invalid)?;
    let signature = Signature::from_slice(&signature).map_err(|_| Error::Invalid)?;
    public
        .verify_strict(parts[..3].join(".").as_bytes(), &signature)
        .map_err(|_| Error::Invalid)?;
    URL_SAFE_NO_PAD.decode(parts[2]).map_err(|_| Error::Invalid)
}
fn identity(
    version: u8,
    product: &str,
    licence_id: &str,
    installation_id: &str,
    installation_key: &str,
    binding: &Binding<'_>,
) -> bool {
    version == 1
        && product == crate::licence_key::PRODUCT
        && uuid(licence_id)
        && uuid(installation_id)
        && licence_id == binding.licence_id
        && installation_id == binding.installation_id
        && installation_key == binding.installation_key
        && URL_SAFE_NO_PAD
            .decode(installation_key)
            .is_ok_and(|key| key.len() == 32)
}
pub fn verify_lease(
    value: &str,
    keys: &LeaseKeys,
    binding: &Binding<'_>,
    now: i64,
) -> Result<Lease, Error> {
    let lease: Lease = serde_json::from_slice(&verified_payload(value, "SWPL1", keys)?)
        .map_err(|_| Error::Invalid)?;
    if !identity(
        lease.version,
        &lease.product,
        &lease.licence_id,
        &lease.installation_id,
        &lease.installation_key,
        binding,
    ) || !epoch(lease.issued_at)
        || !epoch(lease.expires_at)
        || lease.generation == 0
        || lease.generation > 9_007_199_254_740_991
        || !uuid(&lease.lease_id)
        || !uuid(&lease.request_nonce)
        || !action(&lease.action)
        || lease.action == "deactivate"
        || lease.expires_at <= lease.issued_at
        || lease.expires_at - lease.issued_at > OFFLINE_SECONDS
        || lease.issued_at > now.saturating_add(300)
    {
        return Err(Error::Invalid);
    }
    if now >= lease.expires_at {
        return Err(Error::Expired);
    }
    Ok(lease)
}
pub fn verify_reply(
    value: &str,
    keys: &LeaseKeys,
    context: &RequestContext<'_>,
    now: i64,
) -> Result<Reply, Error> {
    let (reply, nonce, reply_action, issued, generation) = if value.starts_with("SWPL1.") {
        let lease = verify_lease(value, keys, &context.binding, now)?;
        let metadata = (
            lease.request_nonce.clone(),
            lease.action.clone(),
            lease.issued_at,
            lease.generation,
        );
        (
            Reply::Lease(lease),
            metadata.0,
            metadata.1,
            metadata.2,
            metadata.3,
        )
    } else {
        let status: Status = serde_json::from_slice(&verified_payload(value, "SWPS1", keys)?)
            .map_err(|_| Error::Invalid)?;
        if !identity(
            status.version,
            &status.product,
            &status.licence_id,
            &status.installation_id,
            &status.installation_key,
            &context.binding,
        ) || !matches!(status.status.as_str(), "revoked" | "deactivated")
            || !epoch(status.issued_at)
            || status.generation == 0
            || status.generation > 9_007_199_254_740_991
            || !uuid(&status.request_nonce)
            || !action(&status.action)
        {
            return Err(Error::Invalid);
        }
        let metadata = (
            status.request_nonce.clone(),
            status.action.clone(),
            status.issued_at,
            status.generation,
        );
        (
            Reply::Status(status),
            metadata.0,
            metadata.1,
            metadata.2,
            metadata.3,
        )
    };
    if nonce != context.nonce
        || reply_action != context.action
        || generation < context.min_generation
        || issued < context.requested_at.saturating_sub(300)
        || issued > now.saturating_add(300)
        || now.saturating_sub(issued) > 600
    {
        return Err(Error::Invalid);
    }
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::{json, Value};
    fn vector() -> Value {
        serde_json::from_str(include_str!(
            "../../tools/licence-keygen/activation-test-vector.json"
        ))
        .unwrap()
    }
    fn keys(v: &Value) -> LeaseKeys {
        LeaseKeys::new(
            vec![(
                "test-lease".into(),
                URL_SAFE_NO_PAD
                    .decode(v["public_key"].as_str().unwrap())
                    .unwrap()
                    .try_into()
                    .unwrap(),
            )],
            &[],
        )
        .unwrap()
    }
    fn context(v: &Value) -> RequestContext<'_> {
        RequestContext {
            binding: Binding {
                licence_id: v["request"]["licence_id"].as_str().unwrap(),
                installation_id: v["request"]["installation_id"].as_str().unwrap(),
                installation_key: v["installation_public_key"].as_str().unwrap(),
            },
            nonce: v["request"]["nonce"].as_str().unwrap(),
            action: "renew",
            requested_at: 1000,
            min_generation: 1,
        }
    }
    fn signed(prefix: &str, claims: Value) -> String {
        let message = format!(
            "{prefix}.test-lease.{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let signature = SigningKey::from_bytes(&[8; 32]).sign(message.as_bytes());
        format!("{message}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
    }
    fn claims(v: &Value, name: &str) -> Value {
        serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(v[name].as_str().unwrap().split('.').nth(2).unwrap())
                .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn verifies_shared_node_vectors_and_installer_request() {
        let v = vector();
        let keys = keys(&v);
        let c = context(&v);
        for name in ["lease", "revoked", "deactivated"] {
            assert!(verify_reply(v[name].as_str().unwrap(), &keys, &c, 1000).is_ok());
        }
        // serde_json's object keys are sorted, matching the server canonical form.
        assert_eq!(
            serde_json::to_string(&v["request"]).unwrap(),
            v["canonical_request"].as_str().unwrap()
        );
        let signature = SigningKey::from_bytes(&[9; 32])
            .sign(v["canonical_request"].as_str().unwrap().as_bytes());
        assert_eq!(
            URL_SAFE_NO_PAD.encode(signature.to_bytes()),
            v["request_signature"].as_str().unwrap()
        );
    }
    #[test]
    fn cached_lease_has_strict_expiry_and_no_unsigned_grace() {
        let v = vector();
        let k = keys(&v);
        let c = context(&v);
        let value = v["lease"].as_str().unwrap();
        assert!(verify_lease(value, &k, &c.binding, 2_592_999).is_ok());
        assert!(matches!(
            verify_lease(value, &k, &c.binding, 2_593_000),
            Err(Error::Expired)
        ));
        let mut payload = claims(&v, "lease");
        payload["expires_at"] = json!(2_593_001);
        assert!(verify_lease(&signed("SWPL1", payload), &k, &c.binding, 1000).is_err());
    }
    #[test]
    fn rejects_wrong_identity_request_generation_roles_and_tampering() {
        let v = vector();
        let k = keys(&v);
        let mut c = context(&v);
        let value = v["lease"].as_str().unwrap();
        c.nonce = "00000000-0000-4000-8000-000000000000";
        assert!(verify_reply(value, &k, &c, 1000).is_err());
        c = context(&v);
        c.min_generation = 2;
        assert!(verify_reply(value, &k, &c, 1000).is_err());
        c = context(&v);
        c.action = "activate";
        assert!(verify_reply(value, &k, &c, 1000).is_err());
        c = context(&v);
        c.binding.installation_id = "00000000-0000-4000-8000-000000000000";
        assert!(verify_reply(value, &k, &c, 1000).is_err());
        c = context(&v);
        assert!(verify_reply(value, &k, &c, 1601).is_err());
        assert!(verify_reply(&(value.to_owned() + "="), &k, &c, 1000).is_err());
        assert!(verify_reply(&value.replace("SWPL1", "SWPC1"), &k, &c, 1000).is_err());
        assert!(verify_reply(&"x".repeat(MAX_BYTES + 1), &k, &c, 1000).is_err());
        assert!(verify_reply(value, &LeaseKeys::default(), &c, 1000).is_err());
        let pair = k.0[0].clone();
        assert!(LeaseKeys::new(vec![pair.clone()], std::slice::from_ref(&pair)).is_err());
        assert!(LeaseKeys::new(vec![pair.clone()], &[("different".into(), pair.1)]).is_err());
        let mut payload = claims(&v, "lease");
        payload["extra"] = json!(true);
        assert!(verify_reply(&signed("SWPL1", payload), &k, &c, 1000).is_err());
        let mut payload = claims(&v, "revoked");
        payload["status"] = json!("licensed");
        assert!(verify_reply(&signed("SWPS1", payload), &k, &c, 1000).is_err());
    }
}
