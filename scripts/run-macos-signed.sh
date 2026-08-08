#!/bin/bash

set -euo pipefail

script_directory="$(cd "$(dirname "$0")" && pwd)"
project_directory="$(cd "${script_directory}/.." && pwd)"
app_path="${project_directory}/src-tauri/target/debug/bundle/macos/Switchify PC.app"
bundle_identifier="com.enaboapps.switchify.pc"
identity_name="Switchify PC Development"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This command is only supported on macOS." >&2
  exit 1
fi

"${script_directory}/setup-macos-dev-signing.sh"
pkill -x switchify-pc 2>/dev/null || true

cd "$project_directory"
npm run tauri -- build --debug --bundles app

codesign --verify --deep --strict --verbose=2 "$app_path"
signing_details="$(codesign -dvv "$app_path" 2>&1)"
requirement="$(codesign -d -r- "$app_path" 2>&1)"

if ! grep -Fq "Authority=${identity_name}" <<<"$signing_details"; then
  echo "The bundle was not signed with ${identity_name}." >&2
  exit 1
fi
if ! grep -Fq "identifier \"${bundle_identifier}\"" <<<"$requirement" || \
   ! grep -Fq "certificate root" <<<"$requirement" || \
   grep -Fq "cdhash" <<<"$requirement"; then
  echo "The bundle does not have the expected stable certificate-based designated requirement:" >&2
  echo "$requirement" >&2
  exit 1
fi

echo "$requirement"
open -n "$app_path"
