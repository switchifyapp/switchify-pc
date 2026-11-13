"""Dependency-free transport tests for the developer-only spike bridge."""

import importlib.util
import io
from pathlib import Path
import struct
import sys
import unittest
from types import SimpleNamespace

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
                return SimpleNamespace(top=lambda n: [SimpleNamespace(word="coffee")])

        source, target = io.BytesIO(), io.BytesIO()
        bridge.send(source, {"before": "I want ", "prefix": "CO"})
        bridge.send(source, {"shutdown": True})
        source.seek(0)
        bridge.serve(Model(), source, target)
        target.seek(0)
        self.assertEqual(bridge.receive(target), {"ready": True})
        self.assertEqual(bridge.receive(target), {"words": ["coffee"]})
        self.assertEqual(bridge.receive(target), {"stopped": True})
        self.assertEqual(calls[0]["left_context"], "i want co")
        self.assertEqual(calls[0]["nbest"], 5)

    def test_rejects_unexpected_or_oversized_context_before_model_call(self):
        for request in (
            {"before": "", "prefix": [], "extra": True},
            {"before": "", "prefix": []},
            {"before": "x" * 1025, "prefix": ""},
        ):
            source = io.BytesIO()
            bridge.send(source, request)
            source.seek(0)
            with self.assertRaises(ValueError):
                bridge.serve(None, source, io.BytesIO())


if __name__ == "__main__":
    unittest.main()
