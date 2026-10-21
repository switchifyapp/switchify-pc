---
title: Licence
description: Switchify PC is free and open source under the GNU Affero General Public License v3, and the parts it includes from other projects keep their own licences.
---

## Switchify PC

Switchify PC is free and open source software. It is licensed under the [GNU Affero General Public License, version 3](https://github.com/switchifyapp/switchify-pc/blob/main/LICENSE) (AGPL v3).

You can use it for free, at home, at school, at work or in a care setting. There is no agreement to accept before you install it.

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
