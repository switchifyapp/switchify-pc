---
title: Account, updates and privacy
description: Optional sign-in that carries your settings between computers, automatic updates, opt-in diagnostics, and what never leaves the computer.
---

## Running in the background

Switchify PC lives in the system tray on Windows or the menu bar on a Mac, with live connection status. From there you can show the window, open Settings and the Switch Forwarding profiles, disconnect devices, pause listening for switch keys, or quit. **Start with system** launches it quietly when you sign in.

**Pause listening** hands your switch keys back to other apps until you choose **Resume listening**. See [Use the switch key as a normal key](/get-started/add-a-switch/#use-the-switch-key-as-a-normal-key).

## Account

Signing in is optional. It uses the same Switchify account as the Android app: enter your email and the six-digit code it receives. There is no password.

**What syncs** between your computers: pointer, repeat, dwell and cursor settings; your switches and what they do; scanning preferences; keyboard layout; Switch Forwarding profiles; and remote switch slots.

**What never syncs:** paired devices and their pairing tokens, the **Approve phones on this account automatically** setting, your diagnostics choice, setup progress, start with system, and learned words.

Changes upload a few seconds after they settle. If two computers change the same thing, the account's copy wins, but an edit is never lost to a deletion. The first sync on a computer whose settings differ asks which to keep.

The Settings sync card on the Account page lists what syncs and what stays on this computer. Under **Remove account** at the bottom of the page, **Delete account…** deletes the account and its saved settings from every device after you confirm.

## Updates

Switchify PC checks for an update shortly after it starts, every six hours, and on demand. Settings shows what is available, downloads it with progress you can cancel, and installs on *Install and restart*.

A release candidate, such as 1.0.0-rc.32, is offered only to people already running a release candidate. A stable release, such as 1.0.0, is offered to everyone, so a release candidate moves to the next stable release when it comes out. A stable install never installs a release candidate, even if one is offered to it.

## Version and licence

The **About** tab in Settings shows which version of Switchify PC you have. It also links to the source code, the [licence and credits](/licence/) and this guide.

## Licence checks

Paid activation is independent of your optional Switchify account. First
activation needs an internet connection. The app checks at launch and daily
while running, and retries failed checks after 15 minutes. Each successful
check grants up to 30 days offline. You can also choose **Check licence now**.

These requests send the purchase key at activation, licence ID, a random
installation ID, an installation public key, request time and a random request
identifier. They do not send typed text, pairing tokens or your account email.
The installation private key stays in the OS credential store. The server stores
activation and transfer records separately from app accounts; deleting an
account does not erase the purchase or free its active computer.

The service uses short-lived hashed network hints for abuse limits. Function
logs contain outcome categories, never purchase keys, signatures or request
bodies. Network infrastructure may process the connection address as part of
serving the request. See the [licence page](/licence/) for offline limits,
transfers and refunds.

## Diagnostics

**Anonymous diagnostics are off until you turn them on.** The setup guide asks you to choose **Share diagnostics** or **Don't share**. If you skipped that, the **Privacy** tab in Settings shows the same two buttons until you choose. Once you have chosen, the Privacy tab shows a single **Share anonymous diagnostic data** switch so you can change your mind at any time. When sharing is on, reports never include typed text, device names or file paths.

Separately, you can **export** a local diagnostics file of up to 500 recent sanitised events at any time, for example to attach to a support email. Nothing is sent unless you send it.

## What stays on the computer

- Typed text. Switchify PC never logs it, and word prediction runs on the computer.
- Pairing tokens, stored in the Windows credential store or the macOS Keychain.
- Learned words for prediction.

Commands from a paired phone travel only over Bluetooth.

The full privacy policy is at [www.theswitchify.app/privacy](https://www.theswitchify.app/privacy#pc-control). Questions: [info@theswitchify.app](mailto:info@theswitchify.app).
