"""Private framed-pipe bridge for the ignored Switchify feasibility test.

Not an installed backend. Never pass real user text to this experiment.
"""

import argparse
import json
import os
from pathlib import Path
import struct
import sys
import threading

LIMIT = 16384
SPELLING = tuple("abcdefghijklmnopqrstuvwxyz'")


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


def serve(model, source, target, keep_alive=False):
    send(target, {"ready": True})
    while True:
        # A bounded lifetime also covers an abandoned WSL launcher. This is
        # only a spike; production would need process-tree supervision.
        timer = None
        try:
            # A live keyboard can remain idle. EOF still exits immediately;
            # only an active request gets a deadline in trial mode.
            if keep_alive:
                request = receive(source)
            timer = threading.Timer(30, lambda: os._exit(1))
            timer.daemon = True
            timer.start()
            if not keep_alive:
                request = receive(source)
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
            if len(before) + len(prefix) > 1024:
                raise ValueError
            words = []
            prefix = prefix.lower()
            # An unsupported prefix is an ordinary empty result, not a broken
            # backend. The next supported query must still work.
            # With no owned prefix, a non-space tail is only a fragment in
            # TextSlinger's word model, not a fresh whole-word boundary.
            boundary_supported = bool(prefix) or not before or before.endswith(" ")
            if boundary_supported and all(character in SPELLING for character in prefix):
                result = model.predict_words(
                    left_context=(before + prefix).lower(),
                    word_spelling_characters=SPELLING,
                    word_ending_characters=(" ",),
                    nbest=5,
                )
                # TextSlinger splits on spaces; Switchify uses Unicode word
                # boundaries. Keep the model's full accepted context, but map
                # only its completion suffix onto Switchify's owned prefix.
                words = [prefix + p.replacement.text for p in result.top(5)]
            send(target, {"id": sequence, "words": words})
        finally:
            if timer is not None:
                timer.cancel()


def load_model(path):
    if not Path(path).is_file():
        raise FileNotFoundError
    from textslinger import NGramLanguageModel

    model = NGramLanguageModel(path)
    # Ready means a usable backend, not just a constructor that returned.
    if not model.predict_words(left_context="the ca", word_spelling_characters=SPELLING,
                               word_ending_characters=(" ",), nbest=5).top(1):
        raise ValueError
    return model


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", required=True)
    parser.add_argument("--keyboard-trial", action="store_true")
    args = parser.parse_args()
    # Preserve a private protocol handle, then redirect fd 1 as well as Python
    # stdout. Even native-library diagnostic writes cannot corrupt frames.
    # The Rust experiment discards stderr; no diagnostics are retained.
    with os.fdopen(os.dup(sys.stdout.fileno()), "wb") as target:
        os.dup2(sys.stderr.fileno(), sys.stdout.fileno())
        model = load_model(args.model)
        serve(model, sys.stdin.buffer, target, keep_alive=args.keyboard_trial)


if __name__ == "__main__":
    try:
        main()
    except EOFError:
        pass
    except Exception:
        # Do not print exceptions: model errors can contain supplied text.
        sys.exit(1)
