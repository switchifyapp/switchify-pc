"""Private framed-pipe bridge for the ignored Switchify feasibility test.

Not an installed backend. Never pass real user text to this experiment.
"""

import argparse
import contextlib
import json
import os
import struct
import sys
import threading

LIMIT = 16384


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


def serve(model, source, target):
    send(target, {"ready": True})
    while True:
        # A bounded lifetime also covers an abandoned WSL launcher. This is
        # only a spike; production would need process-tree supervision.
        timer = threading.Timer(30, lambda: os._exit(1))
        timer.daemon = True
        timer.start()
        try:
            request = receive(source)
            if request == {"shutdown": True}:
                send(target, {"stopped": True})
                return
            if set(request) != {"before", "prefix"}:
                raise ValueError
            before, prefix = request["before"], request["prefix"]
            if not isinstance(before, str) or not isinstance(prefix, str):
                raise ValueError
            if len(before) + len(prefix) > 1024:
                raise ValueError
            # Switchify owns accepted text and edits. This first experiment
            # supports literal-prefix completion, with five word tiles only.
            with contextlib.redirect_stdout(sys.stderr):
                result = model.predict_words(
                    left_context=(before + prefix).lower(),
                    word_spelling_characters=tuple("abcdefghijklmnopqrstuvwxyz'"),
                    word_ending_characters=(" ",),
                    nbest=5,
                )
            send(target, {"words": [p.word for p in result.top(5)]})
        finally:
            timer.cancel()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", required=True)
    args = parser.parse_args()
    # Imports/model loading can emit diagnostic output. It must never enter
    # the framed stdout transport. The Rust experiment discards stderr too.
    with contextlib.redirect_stdout(sys.stderr):
        from textslinger import NGramLanguageModel

        model = NGramLanguageModel(args.model)
    serve(model, sys.stdin.buffer, sys.stdout.buffer)


if __name__ == "__main__":
    try:
        main()
    except EOFError:
        pass
    except Exception:
        # Do not print exceptions: model errors can contain supplied text.
        sys.exit(1)
