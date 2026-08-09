# macOS production releases

Switchify PC is distributed outside the Mac App Store as an Apple Silicon DMG. Production releases use an Apple-issued **Developer ID Application** certificate and Apple notarization. The self-signed `Switchify PC Development` identity remains exclusively for local Accessibility testing through `npm run macos:run`.

## Create and preserve the certificate

1. In Keychain Access, create a certificate signing request using the Apple Developer account email. Select **Saved to disk** and **Let me specify key pair information**, then use RSA 2048-bit keys.
2. In Apple Developer Certificates, create a **Developer ID Application** certificate from that CSR. Download and install the certificate on the Mac that generated the CSR so it joins the existing private key.
3. Verify it appears under **My Certificates** and in `security find-identity -v -p codesigning` with its full `Developer ID Application: … (TEAMID)` identity.
4. Export the certificate and private key together as a password-protected `.p12`. Keep an encrypted offline backup of the `.p12` and its password. Never commit either one.
5. In App Store Connect → Users and Access → Integrations, create a team API key with **Developer** access. Download the `.p8` immediately; Apple only permits downloading it once.

Do not revoke or replace the certificate during normal renewal. Revocation invalidates future signing and can require an emergency rotation. Keep the local `Switchify PC Development`, earlier development, POC, and Preview identities untouched.

## Configure GitHub

Create a `production` environment. Allow deployments from release tags matching `v*` and from `main` for manually dispatched recovery runs. No reviewer gate is required. Add a repository tag ruleset so only maintainers can create or update `v*` tags.

Configure these environment secrets:

| Name | Value |
| --- | --- |
| `APPLE_CERTIFICATE_BASE64` | Base64-encoded contents of the encrypted `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12` |
| `APPLE_API_ISSUER` | App Store Connect API issuer UUID |
| `APPLE_API_KEY` | App Store Connect API key ID |
| `APPLE_API_PRIVATE_KEY` | Complete contents of the downloaded `.p8` file |
| `TIMBERLOGS_API_KEY` | Production telemetry API key |

Configure these environment variables:

| Name | Value |
| --- | --- |
| `APPLE_SIGNING_IDENTITY` | Full `Developer ID Application: … (TEAMID)` identity |
| `APPLE_TEAM_ID` | Apple Developer team ID from the identity |
| `TIMBERLOGS_ENDPOINT` | Production HTTPS telemetry endpoint |

Encode the certificate without line wrapping on macOS:

```bash
base64 -i Switchify-PC-Developer-ID.p12 | tr -d '\n'
```

The workflow writes credentials only beneath the ephemeral runner directory, imports the certificate into a temporary keychain, masks its generated keychain password, and deletes the material during cleanup. Pull-request and `main` CI never receive production signing secrets.

## Publish a release

Keep `package.json` and `src-tauri/tauri.conf.json` versions identical, then create and push the corresponding tag, for example `v1.0.0-beta.1`. The release workflow checks out that exact tag, builds on an Apple Silicon runner, signs and notarizes the app and DMG, validates Gatekeeper and stapled tickets, and creates or updates the matching GitHub Release.

The workflow can be manually dispatched with an existing tag to recover or replace a macOS asset. It never modifies earlier tags or release assets. The published DMG is accompanied by `SHA256SUMS-macos.txt`.

Updater feed generation and updater signing are intentionally separate work. Publishing a DMG does not make the in-app updater functional.

## Rotation and troubleshooting

- Track the Developer ID certificate expiry date and rotate it before expiry by issuing a new certificate, updating all certificate-related secrets together, and validating a release candidate before retiring the old certificate.
- If import fails, verify the `.p12` contains both certificate and private key and that its password is correct.
- If the identity check fails, copy the exact identity from `security find-identity -v -p codesigning`; it must start with `Developer ID Application:` and end with the configured team ID.
- If notarization fails, inspect the Tauri/notarytool output for unsigned nested code, missing hardened runtime, timestamp failures, or rejected entitlements. Do not bypass notarization or stapling.
- If Gatekeeper or stapling validation fails after Apple accepted the upload, rerun the workflow once before replacing credentials; Apple ticket availability can briefly lag.
