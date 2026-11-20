---
title: School and work computers
description: What to ask IT for, why gamepad mode is more reliable, and how to find out why a switch stops working on a managed computer.
---

School and office computers are usually managed by an IT department. Their security settings can stop a switch from reaching Switchify PC, or stop Switchify PC from clicking and typing. This page explains what to ask for and how to work out what is wrong. The [For IT staff](#for-it-staff) section has the technical details an IT department needs to approve it.

## What to ask IT for

Send this list to the IT department before the switch user needs the computer:

1. **Install Switchify PC for all users.** The Windows installer puts it in Program Files for everyone on the computer, which needs administrator rights. Updates need administrator rights too.
2. **Allow it by its publisher.** If the computer only runs approved apps, allow Switchify PC by its code-signing publisher rather than by a file location, so updates keep working.
3. **Leave it alone in the security software.** Switchify PC listens for switch presses everywhere and sends clicks and key presses. Some security software treats that as suspicious and quietly blocks it.
4. **Allow the switch interface.** Some computers block USB or Bluetooth devices they do not know, and some block game controllers on purpose. The switch interface has to be allowed.
5. **On a Mac, allow Accessibility.** Switchify PC needs Accessibility permission. On a managed Mac, IT may have to grant it, because the user cannot.

## Use gamepad mode if you can

If your switch interface has a gamepad or joystick mode, try it on a managed Windows computer. Switchify PC reads gamepad buttons in a different way from keys, and that way is less likely to be blocked by security software or to stop partway through a session. Gamepad buttons never type anything, so they cannot clash with keys other programs use. See [Gamepad mode](/get-started/add-a-switch/) for which buttons work.

Gamepad mode does not help if the computer blocks game controllers. The first check below shows how to tell.

## Find out what is wrong

Work through these in order.

### The switch does nothing anywhere

Open Notepad, or TextEdit on a Mac, and press the switch with Switchify PC closed.

- **In keyboard mode,** the switch should type its key, such as a space. If nothing appears, the computer is not receiving the switch at all.
- **In gamepad mode,** nothing is typed. On Windows, press Windows key + R, type `joy.cpl` and press Enter. The switch interface should be listed under Game Controllers, and its Properties page should light up a button when you press the switch. If it is missing, the computer is not receiving it.

If the computer is not receiving the switch, the problem is the interface, its cable, its mode, or a policy blocking the device. Try another USB port, then ask IT whether USB, Bluetooth or game controller devices are blocked. Switchify PC cannot work around a blocked device.

If the switch works in Notepad but not in Switchify PC, see [Nothing happens when I press the switch](/troubleshooting/#nothing-happens-when-i-press-the-switch) first.

### It works, then stops

If the switch works for a while and then Switchify PC stops responding, security software is the likely cause. Note the time it stopped and ask IT to check their security software's logs for Switchify PC around then.
Trying gamepad mode is also worth it here.

### It stops working in one program

Some programs run as administrator, such as installers, Task Manager and some school management and exam software. Switchify PC from the GitHub releases can still click and type in those, as long as it was installed with its installer into Program Files.

Switch control does not work on the Windows secure desktop, such as the screen that asks "Do you want to allow this app to make changes?". Exam or lockdown software that takes over the whole computer can also block Switchify PC; ask the exam provider whether assistive software is allowed.

### Remote desktop, Citrix or a virtual desktop

If the programs you use run on another computer through Remote Desktop, Citrix or a similar remote session, key presses and game controller buttons usually go to only one of the two computers. Switchify PC only controls the computer it runs on.

- Run Switchify PC on the computer you are sitting at, and it controls that computer, including the remote session's window.
- A game controller usually does not reach the remote computer unless IT sets up USB redirection. Check with IT before installing Switchify PC inside the remote session.

## For IT staff

**Windows**

- **Publisher.** Release installers and the installed programs are signed by **Open Source Developer Owen McGirr**, through a Certum code-signing certificate. Allow by this publisher rather than by path or hash, so updates are allowed too.
- **Install.** The installer is a per-machine NSIS installer that installs to Program Files and needs administrator rights. The in-app updater runs the same installer, so it also needs administrator rights. Startup entries are per user.
- **UIAccess.** Release builds are signed and request UIAccess, which Windows grants only to signed programs in a secure location such as Program Files. This is what lets Switchify PC click and type in programs running as administrator. A copy run from anywhere else does not get it.
- **No administrator rights day to day.** A standard user account can run Switchify PC.
- **How it reads switches.** Keyboard switches are read with a global low-level keyboard hook (`WH_KEYBOARD_LL`). Gamepad and joystick buttons 1 to 8 are read through Raw Input from HID game controller devices. Clicks and key presses are sent with `SendInput`. Security software that flags keyboard hooks or synthetic input may need an exclusion for `switchify-pc.exe` in its Program Files folder. Word prediction runs a bundled Python runtime under `resources\textslinger`. Its programs and libraries are signed by the same publisher, except files Microsoft has already signed.
- **Typed text.** Switchify PC does not log or send what is typed. The diagnostics export in Help contains no typed text, device names or paths.
- **Network.** A licence needs an internet connection for first activation, then works offline for up to 30 days between checks. See [Licence](/licence/). Pairing with an Android phone uses Bluetooth Low Energy.

**Mac**

- **Accessibility.** Switchify PC needs Accessibility permission to read switches and control the pointer and keyboard. On a managed Mac, grant it with a Privacy Preferences Policy Control (PPPC) profile for the bundle identifier `com.enaboapps.switchify.pc`. Run `codesign -dr - "/Applications/Switchify PC.app"` on an installed copy to get the code requirement for the profile.
