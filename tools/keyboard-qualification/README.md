# Native Windows keyboard qualification

This standalone probe tests native OSK activation under signed UIAccess. It is not
the Windows scanning implementation and does not qualify mapped-switch filtering.
Hosted CI compiles it and runs fake lifecycle tests without executing the probe.
Run native tests explicitly with a
disposable editor and other input idle.

## Build and run

With SimplySign authenticated and `SWITCHIFY_CERTUM_CERT_THUMBPRINT` configured:

```powershell
pwsh ./scripts/Build-KeyboardQualification.ps1 -Test -Sign
```

Copy `dist/keyboard-qualification/SwitchifyKeyboardQualification.exe` to
`C:\Program Files\SwitchifyKeyboardQualification`. Only this copy needs elevation.
Run the executable normally, never as administrator. Do not change UIAccess policy
or certificate trust. Quit Switchify for the isolated test and restore its running
state afterward.

The probe logs to `%LOCALAPPDATA%\SwitchifyKeyboardQualification.log`. Save a copy
after every run, because startup overwrites it. Test once with OSK closed, and once
with OSK already open. The probe exits automatically after success, failure, or its
30-second watchdog. Closing the disposable window early cancels further requests.
Verify the owned keyboard disappears after the first run and a pre-existing
keyboard remains after the second. Restore prior keyboard visibility afterward.

The default activation is MSAA `IAccessible.accDoDefaultAction`. UI Automation
discovers a unique visible enabled key; MSAA resolves its center without moving the
pointer and checks process, name, role and bounds before calling the native action.
All accessibility calls run on a separate windowless MTA thread. No focus is forced
back after an invocation. The probe checks focus, text length and the expected
disposable text without logging that text. It uses the native `SC_CLOSE` request
only for its owned OSK, then verifies disappearance rather than assuming a
successful request means the window closed.

For comparison, `--activation=invoke` uses UIA `InvokePattern`. This is a diagnostic
mode, not an automatic fallback. On the tested system it transfers focus to OSK and
the probe cancels immediately.

## Local results, 16 September 2026

Signed x64 testing on Windows 11 Pro 10.0.26200 reported `uiAccess=1` and 84 visible
enabled buttons. Both MSAA runs completed A, A, Space, Enter and Backspace exactly
once, preserved editor focus, and passed expected-text checks. The owned keyboard
disappeared after native close; the already-open keyboard remained running.

The MTA UIA comparison still transferred foreground focus to the OSK process at
the first A. Moving UIA off the editor thread alone did not fix that behavior.

Recorded MSAA key events were down/up pairs with flags 16/144 and `extra=0`. These
are ordinary injected events, not a demonstrated OSK-specific identifier. Do not
filter all injected events or claim mapped-switch recursion is solved. `COMPLETE`
only reports the disposable typing and focus checks. Generated-event identification,
modifier behavior, mapped switches, layouts/displays and Windows ARM remain
unqualified under issue #787.

References: [UIA threading](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-threading),
[MSAA default action](https://learn.microsoft.com/en-us/windows/win32/api/oleacc/nf-oleacc-iaccessible-accdodefaultaction),
[native system close](https://learn.microsoft.com/en-us/windows/win32/menurc/wm-syscommand).
