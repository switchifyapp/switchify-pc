#!/bin/bash

set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: $0 <release-tag> <signing-identity> <team-id>" >&2
  exit 64
fi

release_tag="$1"
signing_identity="$2"
team_id="$3"
project_directory="$(cd "$(dirname "$0")/.." && pwd)"
bundle_directory="${project_directory}/src-tauri/target/aarch64-apple-darwin/release/bundle"

if [[ ! "$release_tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  echo "Invalid release tag: $release_tag" >&2
  exit 1
fi
if [[ "$signing_identity" != 'Developer ID Application: '* ]]; then
  echo 'The release identity must be a Developer ID Application certificate.' >&2
  exit 1
fi
if [[ -z "$team_id" || "$signing_identity" != *"($team_id)" ]]; then
  echo 'The release identity does not match the expected Apple team.' >&2
  exit 1
fi

app_path="$(find "${bundle_directory}/macos" -maxdepth 1 -type d -name '*.app' -print -quit)"
dmg_path="$(find "${bundle_directory}/dmg" -maxdepth 1 -type f -name '*.dmg' -print -quit)"

if [[ -z "$app_path" || ! -d "$app_path" ]]; then
  echo "No application bundle found under ${bundle_directory}/macos." >&2
  exit 1
fi
if [[ -z "$dmg_path" || ! -f "$dmg_path" ]]; then
  echo "No DMG found under ${bundle_directory}/dmg." >&2
  exit 1
fi

main_executable="${app_path}/Contents/MacOS/switchify-pc"
if [[ ! -x "$main_executable" ]]; then
  echo "The expected application executable is missing: $main_executable" >&2
  exit 1
fi

codesign --verify --deep --strict --verbose=2 "$app_path"
codesign --verify --strict --verbose=2 "$dmg_path"

app_details="$(codesign -dvvv "$app_path" 2>&1)"
if ! grep -Fq "Authority=${signing_identity}" <<< "$app_details"; then
  echo 'The application was not signed with the configured Developer ID identity.' >&2
  exit 1
fi
if ! grep -Fq "TeamIdentifier=${team_id}" <<< "$app_details"; then
  echo 'The application signature has the wrong Apple team identifier.' >&2
  exit 1
fi
if ! grep -Eq 'flags=.*\(runtime\)' <<< "$app_details"; then
  echo 'The application signature does not enable hardened runtime.' >&2
  exit 1
fi
if ! grep -Fq 'Timestamp=' <<< "$app_details"; then
  echo 'The application signature does not contain a secure timestamp.' >&2
  exit 1
fi

while IFS= read -r -d '' candidate; do
  if file "$candidate" | grep -Fq 'Mach-O'; then
    candidate_details="$(codesign -dvv "$candidate" 2>&1)"
    if ! grep -Fq "Authority=${signing_identity}" <<< "$candidate_details" || \
       ! grep -Fq "TeamIdentifier=${team_id}" <<< "$candidate_details"; then
      echo "Nested executable uses a different signing identity: $candidate" >&2
      exit 1
    fi
  fi
done < <(find "${app_path}/Contents" -type f -print0)

architectures="$(lipo -archs "$main_executable")"
if [[ "$architectures" != 'arm64' ]]; then
  echo "Expected an Apple Silicon-only executable, found: $architectures" >&2
  exit 1
fi

xcrun stapler validate "$app_path"
xcrun stapler validate "$dmg_path"
spctl --assess --type execute --verbose=4 "$app_path"
spctl --assess --type open --context context:primary-signature --verbose=4 "$dmg_path"

echo "Verified signed, hardened, notarized Apple Silicon release ${release_tag}."
