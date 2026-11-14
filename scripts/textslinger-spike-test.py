"""Dependency-free transport tests for the developer-only spike bridge."""

import importlib.util
import io
from pathlib import Path
import struct
import sys
import tempfile
import time
import unittest
from types import SimpleNamespace
from unittest.mock import patch

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location(
    "bridge", Path(__file__).with_name("textslinger-spike.py")
)
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


class BridgeTest(unittest.TestCase):
    def test_fragmented_frames_and_invalid_lengths(self):
        class Fragmented(io.BytesIO):
            def read(self, count):
                return super().read(min(count, 1))

        stream = Fragmented()
        bridge.send(stream, {"prefix": "café"})
        stream.seek(0)
        self.assertEqual(bridge.receive(stream), {"prefix": "café"})
        for size in (0, bridge.LIMIT + 1):
            with self.assertRaises(ValueError):
                bridge.receive(io.BytesIO(struct.pack("<I", size)))
        with self.assertRaises(EOFError):
            bridge.receive(io.BytesIO(struct.pack("<I", 10) + b"{}"))

    def test_literal_context_and_clean_shutdown(self):
        calls = []

        class Model:
            def predict_words(self, **kwargs):
                calls.append(kwargs)
                return SimpleNamespace(top=lambda n: [
                    SimpleNamespace(replacement=SimpleNamespace(text="ffee"))
                ])

        source, target = io.BytesIO(), io.BytesIO()
        bridge.send(source, {"id": 7, "before": "I want ", "prefix": "CO"})
        bridge.send(source, {"shutdown": True})
        source.seek(0)
        bridge.serve(Model(), source, target)
        target.seek(0)
        self.assertEqual(bridge.receive(target), {"ready": True})
        self.assertEqual(bridge.receive(target), {"id": 7, "words": ["coffee"]})
        self.assertEqual(bridge.receive(target), {"stopped": True})
        self.assertEqual(calls[0]["left_context"], "i want co")
        self.assertEqual(calls[0]["nbest"], 5)

    def test_boundaries_use_completion_suffix_and_unsupported_prefix_recovers(self):
        calls = []

        class Model:
            def predict_words(self, **kwargs):
                calls.append(kwargs["left_context"])
                return SimpleNamespace(top=lambda n: [
                    SimpleNamespace(word="must not use this", replacement=SimpleNamespace(text="ow"))
                ])

        source, target = io.BytesIO(), io.BytesIO()
        fixtures = [("she is well-", "kn"), ('he said "', "he"), ("a,", "b"),
                    ("I am ", "2"), ("", "café"), ('he said "', ""),
                    ("she is well-", ""), ("hello,", ""), ("it costs $", ""),
                    ("look (", ""), ("she is well-", "kn")]
        for sequence, (before, prefix) in enumerate(fixtures, 1):
            bridge.send(source, {"id": sequence, "before": before, "prefix": prefix})
        bridge.send(source, {"shutdown": True})
        source.seek(0)
        bridge.serve(Model(), source, target)
        target.seek(0)
        bridge.receive(target)
        expected = (["know"], ["heow"], ["bow"], [], [], [], [], [], [], [], ["know"])
        for sequence, words in enumerate(expected, 1):
            self.assertEqual(bridge.receive(target), {"id": sequence, "words": words})
        self.assertEqual(calls, ["she is well-kn", 'he said "he', "a,b", "she is well-kn"])

    def test_missing_model_is_rejected_before_import(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(FileNotFoundError):
                bridge.load_model(Path(directory) / "missing.arpa")

    def test_warmup_uses_serving_alphabet(self):
        calls = []
        model = SimpleNamespace(predict_words=lambda **kwargs: (
            calls.append(kwargs) or SimpleNamespace(top=lambda n: ["ready"])))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "model.arpa"
            path.write_text("fixture", encoding="utf-8")
            with patch.dict(sys.modules, {"textslinger": SimpleNamespace(NGramLanguageModel=lambda path: model)}):
                self.assertIs(bridge.load_model(path), model)
        self.assertEqual(calls[0]["word_spelling_characters"], bridge.SPELLING)
        self.assertEqual(calls[0]["word_ending_characters"], (" ",))
        self.assertEqual(calls[0]["nbest"], 5)

    def test_keyboard_mode_waits_for_input_before_starting_timer(self):
        source = io.BytesIO()
        bridge.send(source, {"id": 1, "before": "", "prefix": "2"})
        boundary = source.tell()
        bridge.send(source, {"shutdown": True})
        checks = {0: 0, boundary: 1}
        with patch.object(bridge.threading, "Timer") as timer:
            class CheckedInput(io.BytesIO):
                def read(self, count):
                    if self.tell() in checks:
                        self_expected = checks[self.tell()]
                        if timer.call_count != self_expected:
                            raise AssertionError("idle timer started before input")
                    return super().read(count)
            bridge.serve(None, CheckedInput(source.getvalue()), io.BytesIO(), keep_alive=True)
            self.assertEqual(timer.call_count, 2)

    def test_rejects_unexpected_or_oversized_context_before_model_call(self):
        for request in (
            [],
            {"id": 1, "before": "", "prefix": [], "extra": True},
            {"id": 1, "before": "", "prefix": []},
            {"id": 1, "before": "x" * 1025, "prefix": ""},
            {"id": True, "before": "", "prefix": ""},
            {"id": 0, "before": "", "prefix": ""},
            {"id": 2**64, "before": "", "prefix": ""},
        ):
            source = io.BytesIO()
            bridge.send(source, request)
            source.seek(0)
            with self.assertRaises(ValueError):
                bridge.serve(None, source, io.BytesIO())


def fault_bridge(mode):
    """Synthetic peer for ignored Rust adapter fault-injection tests."""
    source, target = sys.stdin.buffer, sys.stdout.buffer
    bridge.send(target, {"ready": True})
    if mode == "extra":
        bridge.send(target, {"id": 0, "words": ["couple"]})
    while True:
        request = bridge.receive(source)
        if request == {"shutdown": True}:
            bridge.send(target, {"stopped": True})
            return
        if mode == "timeout":
            time.sleep(2.5)
        if mode == "stall":
            time.sleep(10)
        reply = {"id": request["id"], "words": ["coffee"]}
        if mode == "malformed":
            reply["unexpected"] = True
        bridge.send(target, reply)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--fault":
        try:
            fault_bridge(sys.argv[2])
        except (EOFError, BrokenPipeError):
            pass
    else:
        unittest.main()
