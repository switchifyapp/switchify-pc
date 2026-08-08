#!/bin/bash

set -euo pipefail

identity_name="Switchify PC Development"
login_keychain="${HOME}/Library/Keychains/login.keychain-db"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This setup is only supported on macOS." >&2
  exit 1
fi

if security find-identity -v -p codesigning "$login_keychain" 2>/dev/null | grep -Fq "\"${identity_name}\""; then
  echo "Using existing code-signing identity: ${identity_name}"
  exit 0
fi

temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/switchify-signing.XXXXXX")"
trap 'rm -rf "$temporary_directory"' EXIT
certificate_path="${temporary_directory}/certificate.pem"
private_key_path="${temporary_directory}/private-key.pem"
pkcs12_path="${temporary_directory}/identity.p12"
openssl_config="${temporary_directory}/openssl.cnf"
pkcs12_password="$(openssl rand -base64 32)"

cat >"$openssl_config" <<'EOF'
[req]
distinguished_name = subject
x509_extensions = extensions
prompt = no

[subject]
C = IE
O = Enabo Apps
CN = Switchify PC Development

[extensions]
basicConstraints = critical,CA:true
keyUsage = critical,digitalSignature,keyCertSign
extendedKeyUsage = critical,codeSigning
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always
EOF

openssl req -new -x509 -newkey rsa:2048 -sha256 -nodes \
  -days 3650 \
  -config "$openssl_config" \
  -keyout "$private_key_path" \
  -out "$certificate_path"
openssl pkcs12 -export \
  -inkey "$private_key_path" \
  -in "$certificate_path" \
  -name "$identity_name" \
  -passout "pass:${pkcs12_password}" \
  -out "$pkcs12_path"

# -x marks the imported private key as non-extractable. The temporary password and files are
# destroyed when this script exits and are never written inside the repository.
security import "$pkcs12_path" \
  -k "$login_keychain" \
  -P "$pkcs12_password" \
  -x \
  -T /usr/bin/codesign >/dev/null
security add-trusted-cert \
  -r trustRoot \
  -p codeSign \
  -k "$login_keychain" \
  "$certificate_path"

if ! security find-identity -v -p codesigning "$login_keychain" | grep -Fq "\"${identity_name}\""; then
  echo "The identity was imported but is not available to codesign. Check login Keychain access." >&2
  exit 1
fi

echo "Created code-signing identity: ${identity_name}"
