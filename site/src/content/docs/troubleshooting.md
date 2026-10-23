---
title: Troubleshooting
description: The switch is not recognised, input access on a Mac, Bluetooth, and where to get help.
---

Switchify PC's Help page has **Setup** and **Troubleshooting** tabs with the same checks as below, plus illustrated guides for connecting a switch, learning a key, assigning Select, grid and line scanning, press-hold-release, macOS access, pairing and startup. It also links to this guide.

## Nothing happens when I press the switch

- Check the mode on your switch interface. Mouse-click and gamepad modes cannot be learned; choose the keyboard mode.
- Make sure you have added the switch in Settings and it has an action. Automatic scanning needs Select; manual scanning also needs Next and Previous.
- If your interface can be programmed, set it to F13 or higher (F13 to F20 on a Mac). F12 is reserved on Windows.
- On Windows, if a switch was unplugged while held, plug it back in (or use the same key on another keyboard), then press and release it once. That press clears the stuck state and does nothing else; press Select to start again.

## Scanning stopped by itself

Holding any switch for about 4 seconds is the emergency stop; so is Escape. Press Select to start again. The scan also resets after three passes without a selection, and when the display or the window in front changes during a point scan.

When the computer goes to sleep, Switchify PC releases the switch keys and shows **Switches paused while the computer sleeps**. After it wakes, scanning starts again on its own within a few seconds; press Select to begin. If the switches stay quiet after wake, release them, wait a moment and press Select once more, or restart Switchify PC.

If Home shows **Keys released** under Saved switches, someone chose **Release switch keys** in the tray or menu bar. Choose **Resume switch keys** there or on Home, or restart Switchify PC.

## My switch key no longer types in other apps

That is expected while a Select switch is saved: Switchify PC takes the key everywhere. Choose **Release switch keys** in the tray or menu bar to use it as a normal key for a while, then **Resume switch keys** when you are done. See [Use the switch key as a normal key](/get-started/add-a-switch/#use-the-switch-key-as-a-normal-key).

## Input access on a Mac

Choose **Open Accessibility Settings** in Switchify PC. In System Settings, open Privacy & Security, then Accessibility, and turn on Switchify PC. Return to the app and it checks again. If the row is already on but access is still required, remove the row, go back to Switchify PC, reopen Accessibility Settings, and turn on the newly added entry.

## Pairing storage messages

Switchify PC keeps pairing tokens in the Windows credential store or the macOS Keychain. Two messages can appear if that store cannot be used:

- **Pairing storage could not be read**: Switchify PC could not open the credential store or Keychain, so paired phones are not listed or trusted until it can. Your pairings have not been deleted. Restart Switchify PC, or the computer, and the list returns once the store can be read. If the message stays, see [Get help](#get-help).
- **Pair your mobile device again once to finish the secure-storage upgrade**: a pairing saved by an older version no longer has a saved token that Switchify PC can read, so it has been removed. That phone is no longer paired. Pair it once more and it is stored securely.

## Bluetooth

The Troubleshooting tab shows the Bluetooth state and recent changes, and lets you disconnect all devices. Only one app can advertise the Switchify Bluetooth service at a time.

## Get help

Email [info@theswitchify.app](mailto:info@theswitchify.app). Attach the diagnostics export from the Troubleshooting tab if it helps; it contains no typed text, device names or paths. Bugs and ideas are welcome on [GitHub](https://github.com/switchifyapp/switchify-pc/issues).
