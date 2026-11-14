---
title: Word prediction
description: Five word suggestions above the keyboard, from a word model that runs on the computer. Nothing you type is sent anywhere.
---

While you type on the scanning [keyboard](/using/keyboard/), a row of up to five word suggestions appears above it. Select one to insert it and a space.

Suggestions come from the [TextSlinger](https://github.com/kdv123/textslinger) word model, which is included with Switchify PC and runs on your computer. It starts loading when scanning starts, so it is usually ready by the time you open the keyboard. If it is still loading, the row stays empty for a moment; you can type as normal.

The suggestion is typed one character at a time, with a very short gap between characters. This gives slower apps, such as Notepad, time to keep up. A normal word still appears almost at once.

## The suggestion row

Suggestions begin with the first letter you type, and continue after a space as suggestions for the next word. They complete the word you are typing, so each one starts with the letters you have typed so far.

A slot with no suggestion shows a faint dashed outline instead of a word. The outline keeps the slot's place, so the other suggestions stay where they were, and the scan skips it.

When there are no suggestions at all, the row shows five empty outlines and the scan skips the row. Nothing is written across the keyboard. If suggestions stop working, a **Retry predictions** key appears on the keyboard; typing still works.

A long word shrinks a little to fit its slot. A word that is still too wide is split over two lines with a hyphen. Selecting it types the whole word. The same fitting applies to every key label on the keyboard.

Suggestions only change when they need to, so the scan does not restart under you.

## What it suggests

The word model is English. It suggests words made of the letters a to z and apostrophes, such as *can't*. While you are typing a number, an accented letter or another character, the row stays empty until you start a new word.

Suggestions come from the model alone. It does not learn from your typing, and it does not correct mistyped letters. It can sometimes suggest something that is not a real word, or the word you have already typed.

## It stays on the computer

Prediction runs entirely on the computer. Nothing you type leaves it, and no model has to be downloaded. Switchify PC keeps no copy of what you type.

Switchify PC only looks at what you type with its own keyboard. It never reads text fields, documents or other apps.

Earlier versions could learn words from your typing. Switchify PC now deletes any learned words it finds from those versions.

## Capitals

Suggestions follow Shift, Caps and the start of a sentence: the first word after a full stop is capitalised, and a word you start in capitals continues in capitals. The model's words are lowercase, so names and the word *I* are not capitalised on their own; use Shift for those.

## Turning it off

In **Settings > Scanning**, open **More options**, then **Customise keyboard**. Under **Suggestions**, turn off **Word prediction** to hide the row.
