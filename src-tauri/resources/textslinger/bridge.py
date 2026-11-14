"""Switchify PC's word-suggestion helper, run by the bundled TextSlinger runtime.

Speaks framed JSON over private inherited pipes: a 4-byte little-endian length,
then a UTF-8 JSON object of at most LIMIT bytes. It never opens a network
connection and never logs, saves or prints the text it receives.

Requests and replies:
  -> {"id": <1..2**64-1>, "before": <accepted text>, "prefix": <word being typed>}
  <- {"id": <same>, "words": [<up to five completions of prefix>]}
  -> {"shutdown": true}
  <- {"stopped": true}
On start it loads the model and sends {"ready": true}.
"""

import argparse
import json
import os
from pathlib import Path
import struct
import sys
import threading

LIMIT = 16384
# Longest accepted context, in characters, before and including the prefix.
CONTEXT_LIMIT = 1024
# A request that takes longer than this means the helper is stuck; exit so
# Switchify can report suggestions as unavailable. Idle time has no limit.
REQUEST_WATCHDOG_SECONDS = 30
SPELLING = tuple("abcdefghijklmnopqrstuvwxyz'")
SUGGESTIONS = 5


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


def suggestions(model, before, prefix):
    """Up to SUGGESTIONS completions of prefix, lowercase, or none."""
    prefix = prefix.lower()
    # With no prefix, a non-space tail (an opening quote, a hyphen) is a
    # fragment in TextSlinger's word model, not the start of a fresh word.
    boundary_supported = bool(prefix) or not before or before.endswith(" ")
    # An unsupported prefix (digits, accents) is an ordinary empty result, so
    # the next supported word still gets suggestions.
    if not boundary_supported or not all(character in SPELLING for character in prefix):
        return []
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
            if not isinstance(request, dict) or set(request) != {"id", "before", "prefix"}:
                raise ValueError
            sequence = request["id"]
            if type(sequence) is not int or not 0 < sequence < 2**64:
                raise ValueError
            before, prefix = request["before"], request["prefix"]
            if not isinstance(before, str) or not isinstance(prefix, str):
                raise ValueError
            if len(before) + len(prefix) > CONTEXT_LIMIT:
                raise ValueError
            send(target, {"id": sequence, "words": suggestions(model, before, prefix)})
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
        model = load_model(*runtime_model(args.runtime))
        serve(model, sys.stdin.buffer, target)


if __name__ == "__main__":
    try:
        main()
    except EOFError:
        pass
    except Exception:
        # Never print exceptions: they can contain the text being typed.
        sys.exit(1)
