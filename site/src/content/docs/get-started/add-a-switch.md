---
title: Add a switch
description: Connect a jack or USB switch, let Switchify PC learn its key, and choose what a press and a hold do.
---

Switchify PC sees a switch as a key press. Anything that sends a key when you press it will work: a switch interface with a 3.5 mm socket, a USB switch, or a keyboard key.

## Connect the switch

**A jack switch.** Check which connector and socket your switch interface requires, push the plug fully into the interface, and connect the interface to the computer.

**A USB switch.** Connect it to a USB port. Plugging it in does not configure anything by itself; you still add it below.

**Choose the keyboard mode on your interface.** Most interfaces have a mode setting. Mouse-click and gamepad modes cannot be learned, so Switchify PC sees nothing when you press. In keyboard mode most interfaces send Space or Enter, and Switchify PC learns those. If your interface can be programmed, set it to F13 or higher (F13 to F20 on a Mac). Nothing else on the computer types those keys, so they never clash. F12 is reserved on Windows and cannot be used.

## Learn the key

1. In Settings, choose **Switches**, then **Add switch**.
2. Press and release your switch once. The key it sends appears as a badge.
3. Give the switch a name, such as *Head switch* or *Foot pedal*, and save.

While the learning dialog is open it holds keyboard focus, so a press cannot scroll Settings or activate a button. Press Escape to cancel. If you need to type the name with the keyboard, choose **Type with keyboard**, which pauses switch control while you do.

Each key belongs to one switch, but several switches can run the same action.

## Choose what a press does

Give the switch one of these actions for a **press and release**:

- **Select** chooses the highlighted item. Automatic scanning needs one switch set to Select.
- **Next** and **Previous** step through items. Manual scanning needs Select, Next and Previous.
- **Reverse direction**, **Stop scanning**, **Pause / resume**.
- **Open keyboard**, **Open point**, **Open mouse** jump straight to that mode.

The action runs when you release. Scanning movement freezes while any switch is held, so a slow release never selects the wrong thing.

## Hold actions

A switch can also have an ordered list of **hold actions**. Keep the switch down and each action is offered in turn on screen, one per interval. Release while an action is offered to run it. The interval is 1 second by default and can be anything from 250 milliseconds to 5 seconds. Holding on its own never runs anything; only the release does.

## Emergency stop

Press **Escape**, or hold any switch for at least 4 seconds (longer if that switch has a long hold list; Settings shows the exact time), to reset scanning. Switchify PC keeps the switch keys and waits for Select to start again.

## Use the switch key as a normal key

While a Select switch is saved, Switchify PC takes its key everywhere, so a switch set to Space cannot type a space in other apps. To hand the keys back for a while, choose **Release switch keys** in the system tray on Windows or the menu bar on a Mac, or on the Home page under Saved switches. Your switches then do nothing in Switchify PC and the keys work as they normally would.

Choose **Resume switch keys** in the same places to take them back; Home shows the state and a Resume button throughout. The release lasts until you resume or restart Switchify PC, so the switches are always ready after a restart. Switches in Switchify on Android are not affected.

## On Windows

Switchify PC keeps capturing your switch keys while it is in the background or minimised, including when the key is pressed with Shift, Ctrl, Alt or Windows held. Keys you have not assigned pass through to whatever you are using. Switch control does not work on the Windows secure desktop, such as a UAC prompt.
