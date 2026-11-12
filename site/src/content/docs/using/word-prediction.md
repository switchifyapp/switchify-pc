---
title: Word prediction
description: Eight suggestions above the keyboard, computed on the computer and never sent anywhere. Learns from your typing if you let it.
---

While you type on the scanning [keyboard](/using/keyboard/), a row of suggestions appears above it. Select one to insert it and a space.

The suggestion is typed one character at a time, with a very short gap between characters. This gives slower apps, such as Notepad, time to keep up. A normal word still appears almost at once. If you switch to another window while a suggestion is being typed, typing stops.

The three instant word suggestions appear almost as soon as the keyboard opens;
the English word model is a small file that loads quickly. The AI words and
phrases follow once the on-device model has started. The bundled AI worker uses
less memory and reuses calculations while preparing suggestions. It still runs
entirely on your computer.

## What the eight slots are

- **Three instant words** from a statistical model that updates with every letter.
- **Two words from an on-device AI model.**
- **Three two-word phrases** from the same AI model, in wider slots at the end of the row.

A slot with no suggestion shows a faint dashed outline instead of a word. The outline keeps the slot's place, so the other suggestions stay where they were, and the scan skips it.

When there are no suggestions at all, the row shows eight empty outlines and the scan skips the row. Nothing is written across the keyboard. If prediction fails, a **Retry predictions** key appears on the keyboard; typing still works.

A long word shrinks a little to fit its slot. A word that is still too wide is split over two lines with a hyphen. Selecting it types the whole word. The same fitting applies to every key label on the keyboard.

Suggestions only change when they need to, so the scan does not restart under you.

## Nearby-key suggestions

In **Settings > Scanning**, open **More options**, then **Customise keyboard**.
Under **Suggestions**, turn on **Nearby-key suggestions** to allow one nearby
letter change in an unfinished word. It is off by default.

Matching instant completions keep their order. When there are fewer than three,
this option can fill the remaining instant slots. On QWERTY, for example, `hrl`
can produce `help` if a slot is available. Choosing the suggestion replaces the
unfinished word and adds a space. Text changes only when you choose a suggestion.

The setting uses the built-in letter positions for QWERTY, Simple QWERTY or Common
Letters, following your chosen layout. Custom tile positions and switch timing
are outside this model. It applies to unfinished prefixes of two to 32 English
letters. Prefixes containing accents, apostrophes or other characters keep their
usual completion behavior. Generated words and phrases keep their usual matching.

## Abbreviation suggestions

Under **Suggestions**, turn on **Abbreviation suggestions** to type a few letters
of a word instead of its start. It is off by default.

Type the first letter of the word, then some of its other letters in order. For
example, `abt` can produce `about`, `ppl` can produce `people` and `tmrw` can
produce `tomorrow`. Words that start with what you typed keep their places;
abbreviated words only fill instant slots that are left over, ahead of any
nearby-key suggestions. Choosing one replaces the unfinished word and adds a
space. A capital first letter stays a capital. A word typed in capitals, or chosen
with Caps on, is inserted in capitals.

It applies to unfinished words of two to 32 English letters. Words typed with
accents, apostrophes or other characters keep their usual completion behavior.
Generated words and phrases keep their usual matching.

## It stays on the computer

Prediction runs entirely on the computer. Nothing you type leaves it, and no model has to be downloaded. The AI model is included with the app.

Switchify PC only looks at what you type with its own keyboard. It never reads text fields, documents or other apps.

The vocabulary is English.

## Learning from your typing

**Learn from my typing** is on by default. The three instant slots learn the words you use; the AI model does not. What is learned stays on this computer and is not synced to your account. Choose **Forget learned words** in Settings to clear it.

Learned words are stored in a file in Switchify PC's application data folder. If other people use the same computer login and you type something private, forget the learned words afterwards.

## Capitals

Suggestions follow Shift, Caps and the start of a sentence, so *i* becomes *I* and the first word after a full stop is capitalised.
