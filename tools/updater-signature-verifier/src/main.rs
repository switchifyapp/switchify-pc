use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use std::{env, fs, fs::File, io::Read, path::PathBuf};

fn main() {
    if let Err(error) = verify() {
        eprintln!("Updater signature verification failed: {error}");
        std::process::exit(1);
    }
}

fn verify() -> Result<(), String> {
    let mut arguments = env::args_os().skip(1);
    let artifact = PathBuf::from(arguments.next().ok_or("missing artifact path")?);
    let signature_path = PathBuf::from(arguments.next().ok_or("missing signature path")?);
    if arguments.next().is_some() {
        return Err("expected exactly an artifact and signature path".into());
    }

    let public_key_text = env::var("SWITCHIFY_UPDATER_PUBLIC_KEY")
        .map_err(|_| "SWITCHIFY_UPDATER_PUBLIC_KEY is not configured")?;
    let public_key = decode_public_key(&public_key_text)?;
    let signature_text = fs::read_to_string(&signature_path)
        .map_err(|error| format!("cannot read signature file: {error}"))?;
    let signature = decode_signature(&signature_text)?;
    let mut verifier = public_key
        .verify_stream(&signature)
        .map_err(|error| format!("unsupported updater signature: {error}"))?;
    let mut file =
        File::open(&artifact).map_err(|error| format!("cannot open artifact: {error}"))?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read artifact: {error}"))?;
        if count == 0 {
            break;
        }
        verifier.update(&buffer[..count]);
    }
    verifier
        .finalize()
        .map_err(|error| format!("signature does not match artifact: {error}"))
}

fn decode_public_key(value: &str) -> Result<PublicKey, String> {
    PublicKey::decode(value.trim())
        .or_else(|_| PublicKey::from_base64(value.trim()))
        .or_else(|_| {
            let decoded = STANDARD
                .decode(value.trim())
                .map_err(|_| minisign_verify::Error::InvalidEncoding)?;
            let text = std::str::from_utf8(&decoded)
                .map_err(|_| minisign_verify::Error::InvalidEncoding)?;
            PublicKey::decode(text.trim())
        })
        .map_err(|error| format!("invalid updater public key: {error}"))
}

fn decode_signature(value: &str) -> Result<Signature, String> {
    Signature::decode(value.trim())
        .or_else(|_| {
            let decoded = STANDARD
                .decode(value.trim())
                .map_err(|_| minisign_verify::Error::InvalidEncoding)?;
            let text = std::str::from_utf8(&decoded)
                .map_err(|_| minisign_verify::Error::InvalidEncoding)?;
            Signature::decode(text.trim())
        })
        .map_err(|error| format!("invalid updater signature: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_KEY: &str = "untrusted comment: minisign public key E7620F1842B4E81F\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const SIGNATURE: &str = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";

    #[test]
    fn decodes_tauri_wrapped_key_and_signature() {
        let public_key = decode_public_key(&STANDARD.encode(PUBLIC_KEY)).unwrap();
        let signature = decode_signature(&STANDARD.encode(SIGNATURE)).unwrap();
        public_key.verify(b"test", &signature, false).unwrap();
    }

    #[test]
    fn rejects_invalid_wrapped_values() {
        assert!(decode_public_key(&STANDARD.encode("not a public key")).is_err());
        assert!(decode_signature(&STANDARD.encode("not a signature")).is_err());
    }
}
