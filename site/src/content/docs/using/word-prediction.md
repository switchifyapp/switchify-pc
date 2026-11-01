---
title: Word prediction
description: Eight suggestions above the keyboard, computed on the computer and never sent anywhere. Learns from your typing if you let it.
---

While you type on the scanning [keyboard](/using/keyboard/), a row of suggestions appears above it. Select one to insert it and a space.

The three instant word suggestions appear almost as soon as the keyboard opens;
the English word model is a small file that loads quickly. The AI words and
phrases follow once the on-device model has started.

## What the eight slots are

- **Three instant words** from a statistical model that updates with every letter.
- **Two words from an on-device AI model.**
- **Three two-word phrases** from the same AI model, in wider slots at the end of the row.

A slot with no suggestion shows a faint dashed outline instead of a word. The outline keeps the slot's place, so the other suggestions stay where they were, and the scan skips it.

When there are no suggestions at all, the row shows eight empty outlines and the scan skips the row. Nothing is written across the keyboard. If prediction fails, a **Retry predictions** key appears on the keyboard; typing still works.

A long word shrinks a little to fit its slot. Words still fit the slot's width before they wrap onto a second line.

Suggestions only change when they need to, so the scan does not restart under you.

## It stays on the computer

Prediction runs entirely on the computer. Nothing you type leaves it, and no model has to be downloaded. The AI model is included with the app.

Switchify PC only looks at what you type with its own keyboard. It never reads text fields, documents or other apps.

The vocabulary is English.

## Learning from your typing

**Learn from my typing** is on by default. The three instant slots learn the words you use; the AI model does not. What is learned stays on this computer and is not synced to your account. Choose **Forget learned words** in Settings to clear it.

Learned words are stored in a file in Switchify PC's application data folder. If other people use the same computer login and you type something private, forget the learned words afterwards.

## Capitals

Suggestions follow Shift, Caps and the start of a sentence, so *i* becomes *I* and the first word after a full stop is capitalised.
