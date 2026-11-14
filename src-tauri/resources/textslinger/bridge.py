"""Switchify PC's word-suggestion helper, run by the bundled TextSlinger runtime.

Speaks framed JSON over private inherited pipes: a 4-byte little-endian length,
then a UTF-8 JSON object of at most LIMIT bytes. It never opens a network
connection and never logs, saves or prints the text it receives.

Requests and replies:
  -> {"id": <1..2**64-1>, "before": <accepted text>, "prefix": <word being typed>,
      "keys": [[<letter>, <x>, <y>], ...]}
  <- {"id": <same>, "words": [<up to five words>]}
"keys" is optional: where each letter sits on the keyboard, in key widths.
With it, a prefix of two or more letters on those keys is read as presses
that may have landed on a nearby key, so a word can correct the prefix
instead of completing it. Without it, every word completes the prefix.
  -> {"shutdown": true}
  <- {"stopped": true}
On start it loads the model and sends {"ready": true}.
"""

import argparse
import json
import math
import os
from pathlib import Path
import struct
import sys
import threading
import traceback

LIMIT = 16384
# Longest accepted context, in characters, before and including the prefix.
CONTEXT_LIMIT = 1024
# A request that takes longer than this means the helper is stuck; exit so
# Switchify can report suggestions as unavailable. Idle time has no limit.
REQUEST_WATCHDOG_SECONDS = 30
SPELLING = tuple("abcdefghijklmnopqrstuvwxyz'")
SUGGESTIONS = 5
# Nearby-key spread, in key widths. Tight enough that a correctly typed word
# keeps the suggestions it gets without nearby keys; keys further than RADIUS
# from a press are not considered.
SPREAD = 0.3
RADIUS = 1.5
# Prefixes outside this range are completed only. One letter says too little
# to tell a slip from the start of another word, and the search slows as the
# prefix grows: about 0.25 s for eight letters and 0.6 s for twelve, against
# the worker's 1.2 s reply deadline. Keep in step with nearby::correctable.
NEARBY_MIN_PREFIX = 2
NEARBY_MAX_PREFIX = 8
# Each letter at most once, so the list is short.
KEYS_LIMIT = len(SPELLING)


def read_exact(stream, size):
    result = bytearray()
    while len(result) < size:
        chunk = stream.read(size - len(result))
        if not chunk:
            raise EOFError
        result.extend(chunk)
    return bytes(result)


def receive(stream):
    size = struct.unpack("<I", read_exact(stream, 4))[0]
    if not 0 < size <= LIMIT:
        raise ValueError
    return json.loads(read_exact(stream, size))


def send(stream, value):
    payload = json.dumps(value, ensure_ascii=False).encode("utf-8")
    if not 0 < len(payload) <= LIMIT:
        raise ValueError
    stream.write(struct.pack("<I", len(payload)) + payload)
    stream.flush()


def key_positions(keys):
    """{letter: (x, y)} from a request's keys, or ValueError."""
    if not isinstance(keys, list) or len(keys) > KEYS_LIMIT:
        raise ValueError
    positions = {}
    for key in keys:
        if not isinstance(key, list) or len(key) != 3:
            raise ValueError
        letter, x, y = key
        if not isinstance(letter, str) or letter not in SPELLING or letter in positions:
            raise ValueError
        if any(type(v) not in (int, float) or not math.isfinite(v) for v in (x, y)):
            raise ValueError
        positions[letter] = (float(x), float(y))
    return positions


def press(letter, positions):
    """One press of letter, which may have been meant for a nearby key."""
    from textslinger import InputEvent

    px, py = positions[letter]
    alternatives = []
    for key, (x, y) in positions.items():
        distance = (x - px) ** 2 + (y - py) ** 2
        if distance <= RADIUS ** 2:
            alternatives.append((key, -distance / (2 * SPREAD ** 2)))
    # The density of a stray press, which the channel can explain as an extra
    # event: any key, equally.
    return InputEvent(alternatives=tuple(alternatives),
                      channel_insertion_log_density=-math.log(len(positions)))


def suggestions(model, before, prefix, positions=None):
    """Up to SUGGESTIONS lowercase words for prefix, or none. Each completes
    prefix, unless positions are given, when a word may instead correct
    presses that landed on a nearby key."""
    prefix = prefix.lower()
    # With no prefix, a non-space tail (an opening quote, a hyphen) is a
    # fragment in TextSlinger's word model, not the start of a fresh word.
    boundary_supported = bool(prefix) or not before or before.endswith(" ")
    # An unsupported prefix (digits, accents) is an ordinary empty result, so
    # the next supported word still gets suggestions.
    if not boundary_supported or not all(character in SPELLING for character in prefix):
        return []
    if (positions and NEARBY_MIN_PREFIX <= len(prefix) <= NEARBY_MAX_PREFIX
            and all(character in positions for character in prefix)):
        from textslinger import InputChannelConfig

        result = model.predict_words(
            left_context=before.lower(),
            input_sequence=[press(character, positions) for character in prefix],
            input_channel=InputChannelConfig(channel_insertion_probability=0.005,
                                             channel_deletion_probability=0.005),
            word_spelling_characters=SPELLING,
            word_ending_characters=(" ",),
            nbest=SUGGESTIONS,
        )
        return [p.word.lower() for p in result.top(SUGGESTIONS)]
    result = model.predict_words(
        left_context=(before + prefix).lower(),
        word_spelling_characters=SPELLING,
        word_ending_characters=(" ",),
        nbest=SUGGESTIONS,
    )
    # TextSlinger splits words on spaces; Switchify uses Unicode word
    # boundaries. Keep the model's full context, but map only its completion
    # suffix onto the prefix Switchify owns.
    return [prefix + p.replacement.text for p in result.top(SUGGESTIONS)]


def serve(model, source, target):
    send(target, {"ready": True})
    while True:
        # Idle waiting has no deadline; EOF from Switchify exits at once.
        request = receive(source)
        timer = threading.Timer(REQUEST_WATCHDOG_SECONDS, lambda: os._exit(1))
        timer.daemon = True
        timer.start()
        try:
            if request == {"shutdown": True}:
                send(target, {"stopped": True})
                return
            if not isinstance(request, dict) or set(request) - {"keys"} != {"id", "before", "prefix"}:
                raise ValueError
            sequence = request["id"]
            if type(sequence) is not int or not 0 < sequence < 2**64:
                raise ValueError
            before, prefix = request["before"], request["prefix"]
            if not isinstance(before, str) or not isinstance(prefix, str):
                raise ValueError
            if len(before) + len(prefix) > CONTEXT_LIMIT:
                raise ValueError
            positions = key_positions(request["keys"]) if "keys" in request else None
            send(target, {"id": sequence, "words": suggestions(model, before, prefix, positions)})
        finally:
            timer.cancel()


def runtime_model(runtime):
    """The model path and alphabet the runtime's manifest.json declares."""
    runtime = Path(runtime)
    manifest = json.loads((runtime / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("format_version") != 1:
        raise ValueError
    model = runtime / manifest["model"]
    if runtime.resolve() not in model.resolve().parents:
        raise ValueError
    return model, tuple(manifest["model_alphabet"])


def load_model(path, alphabet):
    if not Path(path).is_file():
        raise FileNotFoundError
    from textslinger import NGramLanguageModel

    # A KenLM binary does not record its characters, so the runtime supplies them.
    model = NGramLanguageModel(str(path), model_alphabet=alphabet)
    # Ready means a usable model, not just a constructor that returned.
    if not model.predict_words(left_context="the ca", word_spelling_characters=SPELLING,
                               word_ending_characters=(" ",), nbest=SUGGESTIONS).top(1):
        raise ValueError
    return model


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime", required=True)
    args = parser.parse_args()
    # Keep a private handle for frames, then point fd 1 at stderr, which
    # Switchify discards, so even native-library writes cannot corrupt frames.
    with os.fdopen(os.dup(sys.stdout.fileno()), "wb") as target:
        os.dup2(sys.stderr.fileno(), sys.stdout.fileno())
        try:
            model = load_model(*runtime_model(args.runtime))
        except Exception:
            # Nothing has been received yet, so the error cannot hold typed
            # text. Shown by the packaged runtime check; the app discards it.
            traceback.print_exc()
            sys.exit(1)
        serve(model, sys.stdin.buffer, target)


if __name__ == "__main__":
    try:
        main()
    except EOFError:
        pass
    except Exception:
        # Never print exceptions once serving: they can contain typed text.
        sys.exit(1)
