---
title: Licence
description: Switchify PC is open source under the GNU Affero General Public License v3, and the parts it includes from other projects keep their own licences.
---

## Switchify PC

Switchify PC is open source software. It is licensed under the [GNU Affero General Public License, version 3](https://github.com/switchifyapp/switchify-pc/blob/main/LICENSE) (AGPL v3).

## Purchase keys and the trial

Builds with licensing enabled provide a full 30-day trial on this computer.
A Switchify PC licence covers one computer. When the trial ends,
computer scanning, dwell clicking and mobile control stop. Settings, account
management and licence entry remain available. Ordinary development builds do
not enable paid licensing.

During the trial, a notice at the top of the app shows how many days remain.
It also points to **Settings → Licence** for entering your key. The notice
disappears once this computer is licensed, and returns when 7 days or fewer
remain in its offline allowance.

**Buy licence** opens checkout in your default browser. It appears in the
trial notice and on **Settings → Licence** when this computer is not licensed.
The current link is a Stripe **test checkout**, marked in the app; it does not
take a real payment. An internet connection is needed to open checkout.

Open **Settings → Licence** to paste a key, choose **Paste and activate**, or
choose **Import licence file** to open a saved text licence file. Your trial
days or active licence appear in the status panel at the top of this page.
Connect to the internet for the first activation. No Switchify account is
required. The app checks at launch and daily while running; each successful
check allows up to 30 days offline. The Licence page shows the last successful
check and days remaining. When 7 days or fewer remain, the app shows a reminder,
including at 3 days and 1 day. Choose **Check licence now** after reconnecting.
A failed check leaves the existing allowance intact; it does not add extra days.

Keep a private copy of your purchase key or attachment for reinstalling. If
another computer has the licence, choose **Move licence here**, then confirm.
You can replace an active computer twice in a rolling 30 days; contact support
if you need help after that. The old computer loses access on its next successful
check, or when its offline allowance ends. To free a computer's place first,
choose **Remove licence from this computer** while online and confirm.
Removing a key does not refund a purchase or restart the trial.

A full refund or dispute revokes the licence. A computer stops paid access
when it receives that verified status, or when its existing offline allowance
ends. Settings and licence recovery remain available. If you stay offline
beyond the allowance, connect to the internet and choose **Check licence now**.
Signing out or deleting your Switchify account does not remove the local key.

If the trial or offline allowance has ended, the Licence page automatically moves focus between
buttons every 1.5 seconds. Any configured local switch selects the focused
button. Copy your key first, then choose **Paste and activate**. This recovery
mode controls only this page; it does not type or move the pointer in other
applications. For help or a lost key, contact info@theswitchify.app.

Using the mouse, pressing Tab, or typing in the key field pauses automatic
button scanning on the Licence page. Choose **Resume automatic button
scanning** when you want to use your local switches again.

Live purchases are not available yet. The test checkout is for validating
licence delivery before release.

The licence also lets you study, change and share the app. If you share it, changed or not, the licence sets out how you must make the source code available. A changed version you share must stay under the same licence. If people use your changed version over a network, you must offer them its source code. The [source code](https://github.com/switchifyapp/switchify-pc) is on GitHub. Read the licence for exactly what applies.

## No warranty

Switchify PC is provided as it is, without any warranty, as set out in sections 15 and 16 of the licence. Test it with the person who will use it before relying on it.

## Parts from other projects

Switchify PC includes work from other projects. Each part keeps its own licence. The word prediction parts include their licence files and notices in the installed app; the licences for the other parts are in each project's source code.

- **Word prediction**: [switchify-prediction](https://github.com/switchifyapp/switchify-prediction), MIT licence. Its word list is built from these sources, each under its own terms:
  - English sentences from [Tatoeba](https://tatoeba.org), by way of [WorldAlphabets](https://github.com/AACTools/WorldAlphabets) (CC0 and CC BY 2.0 FR)
  - [Taskmaster-1](https://github.com/google-research-datasets/Taskmaster) by Google LLC ([CC BY 4.0](https://creativecommons.org/licenses/by/4.0/))
  - [The Imagination of Crowds](https://aactext.org/imagine/) AAC sentences by Keith Vertanen and Per Ola Kristensson ([CC BY 4.0](https://creativecommons.org/licenses/by/4.0/))
  - The spoken part of the [Open American National Corpus](https://anc.org/data/oanc/) by Nancy Ide and Keith Suderman, under its publisher's terms
- **AI word suggestions**: the [SmolLM2-135M](https://huggingface.co/HuggingFaceTB/SmolLM2-135M) language model by Hugging Face, Apache License 2.0. It runs in switchify-prediction's AI worker, MIT licence.
- **Bluetooth on a Mac**: [corebluetooth-rs](https://github.com/doom-fish/corebluetooth-rs), MIT or Apache License 2.0.
- **Software libraries**: Switchify PC is built with [Tauri](https://tauri.app), [React](https://react.dev) and many open source Rust and JavaScript libraries. Each keeps its own licence.

The three instant word suggestions are built from the texts above, and from your own typing if **Learn from my typing** is on. The AI words and phrases come from the SmolLM2 model. Any suggestion can sometimes be unsuitable or wrong.
