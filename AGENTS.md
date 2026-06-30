# AGENTS.md

## Project overview

Switchify PC is the Windows-native C#/.NET WPF desktop companion app for Switchify Android. It accepts authenticated Bluetooth commands from the Android app and turns them into PC mouse, keyboard, text, media, window, and status actions.

The app should be treated as a trusted local control agent. Be conservative with networking, authentication, logging, and desktop input behavior.

## Work flow

- Every change needs a GitHub issue before implementation.
- Every change needs its own branch from `main`.
- Branch names should be short and scoped, for example `chore/dotnet-package-1`, `feat/pairing-auth-3`, or `fix/bluetooth-reconnect`.
- Keep pull requests tightly scoped to one issue wherever possible.
- Do not mix implementation work with broad refactors unless the issue explicitly calls for it.
- Prefer existing project patterns once the app scaffold exists.
- Before creating a milestone, verify whether the intended milestone already exists.
- Reuse an existing open milestone instead of creating a duplicate.
- Create a release milestone only when it is missing.
- Do not create future release milestones casually. The active release milestone should normally be the next version after the C# app project `<Version>`.
- Before release work starts, verify the target milestone name exactly matches the version being released, for example `Release 0.2.0` for app project version `0.2.0`.

## Development standards

- Use C#/.NET for app code.
- Keep WPF app composition, core protocol/control logic, and Windows-specific integrations clearly separated.
- Keep OS input execution behind a narrow adapter interface so protocol handling is not tied directly to Win32 APIs.
- Validate all WebSocket messages at runtime before using them.
- Prefer small, testable modules for protocol parsing, pairing, auth, command routing, and desktop input mapping.
- Do not log pairing tokens, shared secrets, raw auth headers, or full typed text payloads.
- Do not add new dependencies without checking whether the existing stack already covers the need.

## Protocol and security

- The PC app must only execute commands from paired Android devices.
- Pairing should use a short-lived code or QR payload and then store a long-lived random shared token.
- Every runtime command must be authenticated before it reaches the desktop input adapter.
- Reject unknown devices, invalid auth, stale timestamps, malformed payloads, and oversized text commands.
- WebSocket errors should return structured error responses and must not crash the app.
- Local-network control is the default assumption. Do not introduce cloud relay behavior unless an issue explicitly requests it.

## Desktop control

- The MVP command set is relative mouse movement, click, right-click, double-click, scroll, key press, shortcut, text typing, media control, and ping.
- Use relative pointer movement first; do not add absolute screen-coordinate mapping unless an issue explicitly requests it.
- Clamp movement, scroll, and text payload sizes to predictable limits.
- Convert native automation failures into structured command errors.
- Manual Windows smoke testing matters for input changes because unit tests should use fake adapters instead of controlling the real OS.

## Windows app guidance

- The app owns Bluetooth lifecycle, pairing/auth state, tray behavior, update checks, and desktop input execution.
- The UI shows pairing, connection status, Bluetooth status, update state, and recent non-sensitive errors.
- Keep the app useful from the tray: show window, server status, disconnect, and quit.
- Avoid blocking the UI thread with long-running work.

## Design language

- Use sentence case for user-facing text: headings, navigation, buttons, labels, placeholders, and status.
- Use `...` for in-progress status text until the project standardizes typography.
- Terminal and empty states should end with a period, for example `No devices connected.`
- Keep the UI quiet and utilitarian. This is a background companion utility, not a marketing surface.
- Do not show secrets or token fragments in the UI.

## Testing

Before pushing implementation changes, run the most relevant checks available for the current scaffold. As the repo matures, this should normally include:

```powershell
dotnet restore src/SwitchifyPc.sln
dotnet build src/SwitchifyPc.sln -c Release --no-restore
dotnet test src/SwitchifyPc.sln -c Release --no-build
```

For desktop input or packaging changes, also run the relevant manual smoke path:

- App launches.
- Tray menu works.
- Pairing/status UI opens.
- WebSocket server starts.
- Authenticated ping receives an ack.
- Mouse move, click, right-click, scroll, keyboard shortcut, and text entry work on Windows.

## Release flow

Only the maintainer should publish releases.

Release builds are published from tags named `vX.Y.Z`, where `X.Y.Z` must match `src/SwitchifyPc.App/SwitchifyPc.App.csproj` `<Version>`.

Before creating a release milestone, verify whether the intended milestone already exists. Reuse an existing open milestone. Create the milestone only if it is missing. If the milestone exists but is closed, reopen it only when the work truly belongs in that release.

The release workflow is `.github/workflows/release.yml`. It runs on the self-hosted Windows signing runner with the `switchify-signing` label. The runner is expected to have:

- Windows.
- .NET SDK.
- GitHub CLI authentication available to the workflow.
- Windows SDK signing tools available, including `signtool`.
- The Certum SimplySign certificate available in `Cert:\CurrentUser\My`.
- An active SimplySign session before the release job runs.

Production release signing uses Certum SimplySign through `signtool`. Do not document or commit the real certificate thumbprint. Use placeholders in docs and examples.

Required signing environment for production packaging:

```powershell
$env:SWITCHIFY_SIGNING_MODE = "certum-simplysign"
$env:SWITCHIFY_CERTUM_CERT_THUMBPRINT = "<certum-certificate-thumbprint>"
$env:SWITCHIFY_CERTUM_TIMESTAMP_URL = "http://time.certum.pl"
```

### Release preflight

Before creating or pushing a release tag, the maintainer or agent must verify all of the following and stop if any check fails:

- Local `main` is clean and up to date with `origin/main`.
- The C# app project `<Version>` is the exact version being released.
- The release tag is exactly `vX.Y.Z`, where `X.Y.Z` equals the C# app project `<Version>`.
- The release issue and release PR are assigned to milestone `Release X.Y.Z`.
- The milestone `Release X.Y.Z` exists and has no unrelated open issues.
- The self-hosted runner with label `switchify-signing` is online and not busy.
- The Certum signing certificate thumbprint is available out-of-band or through the configured repository variable.
- The matching certificate exists in `Cert:\CurrentUser\My`, has a private key, and SimplySign is logged in.

Use commands like these for preflight checks. Do not print, document, or commit the real certificate thumbprint.

```powershell
[xml]$project = Get-Content src/SwitchifyPc.App/SwitchifyPc.App.csproj
$projectVersion = $project.Project.PropertyGroup.Version | Select-Object -First 1
$expectedTag = "v$projectVersion"
$expectedMilestone = "Release $projectVersion"

git status --short --branch
git pull --ff-only

$milestones = gh api repos/switchifyapp/switchify-pc/milestones?state=all | ConvertFrom-Json
$milestone = $milestones | Where-Object { $_.title -eq $expectedMilestone } | Select-Object -First 1
if (-not $milestone) {
  throw "Expected milestone was not found: $expectedMilestone"
}

$runner = (gh api repos/switchifyapp/switchify-pc/actions/runners | ConvertFrom-Json).runners |
  Where-Object { $_.labels.name -contains 'switchify-signing' } |
  Select-Object -First 1

if (-not $runner -or $runner.status -ne 'online' -or $runner.busy) {
  throw 'The switchify-signing runner is not ready.'
}

$thumbprint = $env:SWITCHIFY_CERTUM_CERT_THUMBPRINT -replace '[^a-fA-F0-9]', ''
if (-not $thumbprint) {
  throw 'SWITCHIFY_CERTUM_CERT_THUMBPRINT is not set in this shell. Do not hard-code it.'
}

$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
  Where-Object { $_.Thumbprint -eq $thumbprint } |
  Select-Object -First 1

if (-not $cert -or -not $cert.HasPrivateKey) {
  throw 'The Certum signing certificate is missing or has no private key.'
}
```

When preparing the next milestone, verify first and create only if missing:

```powershell
$nextMilestone = "Release X.Y.Z"
$milestones = gh api repos/switchifyapp/switchify-pc/milestones?state=all | ConvertFrom-Json
$existing = $milestones | Where-Object { $_.title -eq $nextMilestone } | Select-Object -First 1

if (-not $existing) {
  gh api repos/switchifyapp/switchify-pc/milestones -f title=$nextMilestone
}
```

If the thumbprint is available only as a GitHub repository variable, the maintainer may verify that the variable exists without printing its value, then verify the local certificate with the thumbprint supplied privately.

The release workflow:

- restores the C# solution
- builds the C# solution
- runs C# tests
- verifies the Certum signing certificate
- packages the Windows x64 NSIS installer with `pwsh scripts/Package-Windows.ps1`
- verifies updater metadata
- verifies the tag matches the C# app project `<Version>`
- uploads all top-level `dist` release assets to GitHub Releases, including the signed installer and updater metadata

Local packaging with `pwsh scripts/Package-Windows.ps1` creates artifacts under `dist`. It does not publish a GitHub release.

Before publishing a release, the maintainer should run or confirm the Windows smoke path:

- signed installer verifies with Authenticode
- installer installs per-machine under `C:\Program Files\Switchify PC\`
- `pwsh scripts/Verify-DotnetPackage.ps1` passes
- `pwsh scripts/Verify-UpdaterMetadata.ps1` passes
- app launches and stays running
- tray menu works
- Bluetooth pairing works
- authenticated ping works
- mouse, click, right-click, scroll, keyboard/text, media, and window control commands work
- Settings > Updates can check, download, and show `Install update` in packaged builds
- updater metadata exists in `dist\latest.yml` and packaged `resources\app-update.yml`

To publish a release, the maintainer may push an annotated tag:

```powershell
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

The workflow can also be dispatched manually by the maintainer with a `tag` input.

Do not push a release tag until the release PR with the matching version bump has merged to `main` and local `main` has been pulled.

Do not tag if:

- The C# app project `<Version>` does not match the intended tag.
- The release issue or PR milestone does not match `Release X.Y.Z`.
- The signing runner is offline or busy.
- The Certum certificate cannot be verified locally.

After the release workflow succeeds:

1. Verify the GitHub Release exists for `vX.Y.Z`.
2. Verify the release assets are present:
   - `Switchify-PC-Setup-X.Y.Z-x64.exe`
   - `latest.yml`
   - `builder-debug.yml`
3. Verify there are no open issues in milestone `Release X.Y.Z`.
4. Close milestone `Release X.Y.Z`.
5. If issues remain open, do not close the milestone; report the blockers.

Use commands like these for post-release verification and milestone closure:

```powershell
gh release view $expectedTag --repo switchifyapp/switchify-pc

$openIssues = gh issue list `
  --repo switchifyapp/switchify-pc `
  --milestone $expectedMilestone `
  --state open `
  --json number,title,state | ConvertFrom-Json

if ($openIssues.Count -gt 0) {
  throw "Milestone $expectedMilestone still has open issues."
}

gh api "repos/switchifyapp/switchify-pc/milestones/$($milestone.number)" `
  -X PATCH `
  -f state=closed `
  --silent
```

Do not:

- publish releases from contributor machines
- tag releases without maintainer intent
- hard-code a real certificate thumbprint in docs
- bypass the signed Windows runner for production installers
- publish unsigned production installers
- change signing, updater, WinGet, or cross-platform release behavior unless a dedicated issue explicitly scopes it
