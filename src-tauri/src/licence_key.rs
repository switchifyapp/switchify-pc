//! Version 1 offline licences. Verify the exact envelope before parsing claims.
//! No signing secrets or production trust anchors belong in this module.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const PRODUCT: &str = "switchify-pc";
pub const MAX_KEY_BYTES: usize = 2048;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    pub version: u8,
    pub licence_id: String,
    pub product: String,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub seats: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    Invalid,
    UnknownIssuer,
    Expired,
    NotYetValid,
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Invalid => "This licence key is invalid.",
            Self::UnknownIssuer => "This licence requires a newer version of Switchify PC.",
            Self::Expired => "This licence has expired.",
            Self::NotYetValid => "This licence is not valid yet. Check your computer's date.",
        })
    }
}

pub fn valid_key_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// `trusted_keys` comes from the signed application's compiled trust anchors,
/// never from the licence, a preferences file, or a server response.
pub fn verify(
    key: &str,
    trusted_keys: &[(&str, [u8; 32])],
    now_seconds: i64,
) -> Result<Claims, KeyError> {
    let key = key.trim();
    if key.len() > MAX_KEY_BYTES {
        return Err(KeyError::Invalid);
    }
    let parts: Vec<_> = key.split('.').collect();
    if parts.len() != 4 || parts[0] != "SWPC1" || !valid_key_id(parts[1]) {
        return Err(KeyError::Invalid);
    }
    let (_, public) = trusted_keys
        .iter()
        .find(|(id, _)| *id == parts[1])
        .ok_or(KeyError::UnknownIssuer)?;
    let public = VerifyingKey::from_bytes(public).map_err(|_| KeyError::Invalid)?;
    let signature_bytes = URL_SAFE_NO_PAD
        .decode(parts[3])
        .map_err(|_| KeyError::Invalid)?;
    let signature = Signature::from_slice(&signature_bytes).map_err(|_| KeyError::Invalid)?;
    let signed = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    public
        .verify_strict(signed.as_bytes(), &signature)
        .map_err(|_| KeyError::Invalid)?;
    let payload = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| KeyError::Invalid)?;
    let claims: Claims = serde_json::from_slice(&payload).map_err(|_| KeyError::Invalid)?;
    if claims.version != 1
        || claims.product != PRODUCT
        || uuid::Uuid::parse_str(&claims.licence_id).is_err()
        || claims.issued_at < 0
        || claims.issued_at > 253_402_300_799
        || claims.seats == 0
        || claims.seats > 1_000_000
        || claims
            .expires_at
            .is_some_and(|expires| expires <= claims.issued_at || expires > 253_402_300_799)
    {
        return Err(KeyError::Invalid);
    }
    // A small allowance for clock skew; expiry has no grace period.
    if now_seconds.saturating_add(300) < claims.issued_at {
        return Err(KeyError::NotYetValid);
    }
    if claims
        .expires_at
        .is_some_and(|expires| now_seconds >= expires)
    {
        return Err(KeyError::Expired);
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn signed(payload: &str) -> (String, [u8; 32]) {
        let signer = SigningKey::from_bytes(&[7; 32]);
        let message = format!("SWPC1.test.{}", URL_SAFE_NO_PAD.encode(payload));
        let signature = signer.sign(message.as_bytes());
        (
            format!("{message}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes())),
            signer.verifying_key().to_bytes(),
        )
    }

    const PAYLOAD: &str = r#"{"version":1,"licence_id":"6af44000-17b7-4d87-ad1d-32e9a31b331a","product":"switchify-pc","issued_at":1000,"expires_at":2000,"seats":1}"#;

    #[test]
    fn verifies_node_generated_compatibility_vector() {
        let vector: serde_json::Value =
            serde_json::from_str(include_str!("../../tools/licence-keygen/test-vector.json"))
                .unwrap();
        let public: [u8; 32] = URL_SAFE_NO_PAD
            .decode(vector["public_key"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let claims = verify(vector["key"].as_str().unwrap(), &[("test", public)], 1000).unwrap();
        assert_eq!(claims.seats, 1);
        assert_eq!(claims.product, PRODUCT);
    }

    #[test]
    fn rejects_tampering_unknown_keys_and_bad_encoding() {
        let (key, public) = signed(PAYLOAD);
        assert!(verify(&key, &[("test", public)], 1000).is_ok());
        assert_eq!(verify(&key, &[], 1000), Err(KeyError::UnknownIssuer));
        assert!(verify(&key.replace("SWPC1", "SWPC2"), &[("test", public)], 1000).is_err());
        assert!(verify(&key.replace("test.", "other."), &[("other", public)], 1000).is_err());
        assert!(verify(&(key.clone() + "="), &[("test", public)], 1000).is_err());
        assert!(verify(&"x".repeat(MAX_KEY_BYTES + 1), &[("test", public)], 1000).is_err());
        assert!(verify(&key, &[("test", [0; 32])], 1000).is_err());
    }

    #[test]
    fn checks_time_and_claims_after_authenticating() {
        let (key, public) = signed(PAYLOAD);
        assert_eq!(
            verify(&key, &[("test", public)], 2000),
            Err(KeyError::Expired)
        );
        assert_eq!(
            verify(&key, &[("test", public)], 699),
            Err(KeyError::NotYetValid)
        );
        for payload in [
            PAYLOAD.replace("switchify-pc", "other"),
            PAYLOAD.replace("\"seats\":1", "\"seats\":0"),
            PAYLOAD.replace("\"version\":1", "\"version\":2"),
            PAYLOAD.replace("\"version\":1", "\"version\":1,\"version\":1"),
            PAYLOAD.replace("\"seats\":1", "\"seats\":1,\"unknown\":true"),
        ] {
            let (key, public) = signed(&payload);
            assert_eq!(
                verify(&key, &[("test", public)], 1000),
                Err(KeyError::Invalid)
            );
        }
    }
}
