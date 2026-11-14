#!/usr/bin/env bash
# Signs every Mach-O file in the bundled TextSlinger runtime with the release
# Developer ID, hardened runtime and a secure timestamp, before Tauri copies it
# into the app. Notarization rejects any nested binary that is not signed this
# way, and scripts/verify-macos-release.sh checks the identity of each one.
#
# Usage: scripts/sign-macos-textslinger-runtime.sh <signing identity> [keychain]
set -euo pipefail

identity="${1:?signing identity required}"
keychain="${2:-}"
runtime="src-tauri/resources/textslinger/runtime"

if [[ ! -f "${runtime}/manifest.json" ]]; then
  echo "No TextSlinger runtime at ${runtime}; run npm run prediction-model first." >&2
  exit 1
fi

keychain_args=()
if [[ -n "$keychain" ]]; then
  keychain_args=(--keychain "$keychain")
fi

signed=0
while IFS= read -r -d '' candidate; do
  if file -b "$candidate" | grep -q 'Mach-O'; then
    codesign --force --options runtime --timestamp --sign "$identity" "${keychain_args[@]}" "$candidate"
    codesign --verify --strict "$candidate"
    signed=$((signed + 1))
  fi
done < <(find "$runtime" -type f -print0)

if [[ "$signed" -eq 0 ]]; then
  echo 'No Mach-O files were found in the TextSlinger runtime.' >&2
  exit 1
fi
echo "Signed ${signed} Mach-O files in the TextSlinger runtime."
