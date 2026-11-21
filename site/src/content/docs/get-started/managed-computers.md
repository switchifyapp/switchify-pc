---
title: School and work computers
description: What to ask IT for, when gamepad mode can help, and how to find out why a switch stops working on a managed computer.
---

School and office computers are usually managed by an IT department. Their security settings can stop a switch from reaching Switchify PC, or stop Switchify PC from clicking and typing. This page explains what to ask for and how to work out what is wrong. The [For IT staff](#for-it-staff) section has the technical details an IT department needs to approve it.

## What to ask IT for

Send this list to the IT department before the switch user needs the computer:

1. **Install Switchify PC for all users.** The Windows installer puts it in Program Files for everyone on the computer, which needs administrator rights. Updates need administrator rights too, so a switch user on a standard account cannot install them; IT needs to.
2. **Allow it by its publisher.** If the computer only runs approved apps, allow Switchify PC by its code-signing publisher rather than by a file location, so updates keep working.
3. **Add it to the security software's allowed list.** Switchify PC listens for switch presses everywhere and sends clicks and key presses. Some security software treats that as suspicious and quietly blocks it.
4. **Allow the switch interface.** Some computers block USB or Bluetooth devices they do not know, and some block game controllers on purpose. The switch interface has to be allowed.
5. **On a Mac, allow Accessibility.** Switchify PC needs Accessibility permission. On a managed Mac, IT may have to grant it, because the user cannot.

## When gamepad mode can help

If your switch interface has a gamepad or joystick mode, it is worth trying on a managed Windows computer. Gamepad buttons are read separately from keyboard keys, so they may keep working when security software interferes with keyboard switches. Remove any keyboard switches from Switchify PC while you try it, because a keyboard switch that stops working stops scanning for every switch. Gamepad buttons never type anything, so they cannot clash with keys other programs use. See [Gamepad mode](/get-started/add-a-switch/#connect-the-switch) for which buttons work.

Gamepad mode does not help if the computer blocks game controllers, if security software stops Switchify PC's keyboard listener from starting, or if it blocks Switchify PC altogether. The first check below shows how to tell.

## Find out what is wrong

Work through these in order.

### The switch does nothing anywhere

First stop Switchify PC from taking the switch: choose **Pause listening** in the tray (the menu bar on a Mac), or quit Switchify PC from there. Closing its window is not enough, because it keeps running in the tray. Then open Notepad, or TextEdit on a Mac, and press the switch.

- **In keyboard mode,** a switch that sends a key such as Space or Enter should type it. If nothing appears, the computer is not receiving the switch. Keys F13 and higher type nothing even when they work, so for those, choose **Add switch**, then **Learn key**, in Switchify PC instead and see whether it learns the key.
- **In gamepad mode on Windows,** nothing is typed. Press Windows key + R, type `joy.cpl` and press Enter. The switch interface should be listed under Game Controllers, and its Properties page should light up a button when you press the switch. On a Mac, choose **Add switch**, then **Learn key**, in Switchify PC and see whether it learns the button.

If the computer is not receiving the switch, the problem is the interface, its cable, its mode, or a policy blocking the device. Try another USB port, then ask IT whether USB, Bluetooth or game controller devices are blocked. Switchify PC cannot work around a blocked device.

When you are done, choose **Resume listening**, or start Switchify PC again if you quit it. If the switch works in Notepad but not in Switchify PC, see [Nothing happens when I press the switch](/troubleshooting/#nothing-happens-when-i-press-the-switch).

### It works, then stops

First check [Scanning stopped by itself](/troubleshooting/#scanning-stopped-by-itself). Holding a switch, sleep and **Pause listening** all stop scanning on purpose.

If none of those apply, security software may be the cause. Note the time it stopped and ask IT to check their security software's logs for Switchify PC around then. On Windows, gamepad mode is worth trying too.

### On Windows, it stops working in one program

Some programs run as administrator, such as installers, Task Manager and some school management and exam software. Switchify PC installed with its installer can still click and type in those. A copy run from somewhere else cannot.

Switch control does not work on the Windows secure desktop, such as the screen that asks "Do you want to allow this app to make changes?". Exam or lockdown software that takes over the whole computer may also block Switchify PC; ask the exam provider whether assistive software is allowed.

### Remote desktop, Citrix or a virtual desktop

Switchify PC controls the computer it runs on. If the programs you use run on another computer through Remote Desktop, Citrix or a similar remote session, it may matter which computer the switch and Switchify PC are on. Remote sessions do not always pass key presses or game controllers through. If the switch does not work in the session, ask IT which computer Switchify PC should be installed on and whether the switch interface can be passed through to the session.

## For IT staff

**Windows**

- **Publisher.** Release installers and the installed programs are signed by **Open Source Developer Owen McGirr**, through a Certum code-signing certificate. Allow by this publisher rather than by path or hash, so updates are allowed too.
- **Install.** The installer is a per-machine NSIS installer that installs to Program Files and needs administrator rights. The in-app updater runs the same installer, so it also needs administrator rights. If Switchify PC is open for another signed-in account when an update, install or uninstall runs, the installer closes that copy, because the program files are shared. Interactive installs and uninstalls ask first. In-app updates, and installs run in passive or silent mode, close it without asking. That person needs to start Switchify PC again. Startup entries are per user.
- **UIAccess.** Release builds are signed and request UIAccess, which Windows grants only to signed programs in a secure location such as Program Files. This is what lets Switchify PC click and type in programs running as administrator.
- **Accounts.** The release manifest requests `asInvoker` with UIAccess, so Switchify PC starts without a UAC prompt and without elevation on standard accounts and on administrator accounts with UAC on, including Start with system. Versions before 1.16.1 requested `highestAvailable`, which prompted at every launch on administrator accounts and could not capture switches on standard accounts; use 1.16.1 or later.
- **Programs.** `switchify-pc.exe` is the app. It starts a second copy of itself to capture switch input, and another for word prediction, through the Shell so they get UIAccess without elevation. The copies talk over named pipes that only the signed-in user and SYSTEM can open. `switchify-pc-startup.exe` starts it when the user signs in, if Start with system is on. Word prediction runs a bundled Python runtime under `resources\textslinger\runtime`. These are signed by the same publisher, except files their makers had already signed, such as Microsoft's C runtime and some Python Software Foundation files. Rules that also check DLLs need to allow those publishers too.
- **How it reads switches.** Every session installs a global low-level keyboard hook (`WH_KEYBOARD_LL`) and registers for keyboard Raw Input. Gamepad and joystick buttons 1 to 8 are read through Raw Input from HID game controller devices. Clicks and key presses are sent with `SendInput`. Security software that flags keyboard hooks or synthetic input may need an exclusion for these programs.
- **Typed text.** Switchify PC does not log or send what is typed. The diagnostics export in Help contains no typed text, device names or paths.
- **Network.** A licence needs an internet connection for first activation, then works offline for up to 30 days between checks. See [Licence](/licence/). Update checks, the optional Switchify account and settings sync, and diagnostics sharing (off unless the user turns it on) also use the internet. See [Account, updates and privacy](/account-and-privacy/).

**Mac**

- **Accessibility.** Switchify PC needs Accessibility permission to read switches and control the pointer and keyboard. On a managed Mac, grant it with a Privacy Preferences Policy Control (PPPC) profile for the bundle identifier `com.enaboapps.switchify.pc`. Run `codesign -dr - "/Applications/Switchify PC.app"` on an installed copy to get the code requirement for the profile.
- **Automation.** Shutting down or restarting the Mac from Switchify PC needs permission to control System Events. The same PPPC profile can allow it.
- **Bluetooth.** Pairing with Switchify on Android needs Bluetooth permission, which the user grants when asked.
