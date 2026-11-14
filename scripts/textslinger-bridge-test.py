"""Dependency-free tests for the bundled TextSlinger bridge.

Run with any Python 3.10 or newer: python scripts/textslinger-bridge-test.py
`--fault <mode>` runs a synthetic peer for the Rust adapter's fault tests.
"""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import time
import unittest
from types import SimpleNamespace
from unittest.mock import patch

sys.dont_write_bytecode = True

BRIDGE = Path(__file__).resolve().parent.parent / "src-tauri/resources/textslinger/bridge.py"
spec = importlib.util.spec_from_file_location("bridge", BRIDGE)
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


def completions(*suffixes):
    return lambda **kwargs: SimpleNamespace(top=lambda n: [
        SimpleNamespace(replacement=SimpleNamespace(text=suffix)) for suffix in suffixes
    ])


class Cancelled(Exception):
    """Stands in for TextSlinger's PredictionCancelled."""


class Transport(unittest.TestCase):
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

    def test_oversized_reply_is_refused(self):
        with self.assertRaises(ValueError):
            bridge.send(io.BytesIO(), {"words": ["x" * bridge.LIMIT]})


class Serving(unittest.TestCase):
    def run_requests(self, model, requests):
        source, target = io.BytesIO(), io.BytesIO()
        for request in requests:
            bridge.send(source, request)
        bridge.send(source, {"shutdown": True})
        source.seek(0)
        bridge.serve(model, source, target)
        target.seek(0)
        replies = []
        while target.tell() < len(target.getvalue()):
            replies.append(bridge.receive(target))
        return replies

    def test_literal_context_and_clean_shutdown(self):
        calls = []

        class Model:
            def predict_words(self, **kwargs):
                calls.append(kwargs)
                return completions("ffee")()

        replies = self.run_requests(Model(), [{"id": 7, "before": "I want ", "prefix": "CO"}])
        self.assertEqual(replies, [{"ready": True}, {"id": 7, "words": ["coffee"]}, {"stopped": True}])
        self.assertEqual(calls[0]["left_context"], "i want co")
        self.assertEqual(calls[0]["nbest"], 5)

    def test_boundaries_use_completion_suffix_and_unsupported_prefix_recovers(self):
        calls = []

        class Model:
            def predict_words(self, **kwargs):
                calls.append(kwargs["left_context"])
                return completions("ow")()

        fixtures = [("she is well-", "kn"), ('he said "', "he"), ("a,", "b"),
                    ("I am ", "2"), ("", "café"), ('he said "', ""),
                    ("she is well-", ""), ("hello,", ""), ("it costs $", ""),
                    ("look (", ""), ("she is well-", "kn")]
        requests = [{"id": i, "before": b, "prefix": p} for i, (b, p) in enumerate(fixtures, 1)]
        replies = self.run_requests(Model(), requests)[1:-1]
        expected = (["know"], ["heow"], ["bow"], [], [], [], [], [], [], [], ["know"])
        self.assertEqual(replies, [{"id": i, "words": w} for i, w in enumerate(expected, 1)])
        self.assertEqual(calls, ["she is well-kn", 'he said "he', "a,b", "she is well-kn"])

    def test_rejects_unexpected_or_oversized_context_before_model_call(self):
        for request in (
            [],
            {"id": 1, "before": "", "prefix": [], "extra": True},
            {"id": 1, "before": "", "prefix": []},
            {"id": 1, "before": "x" * (bridge.CONTEXT_LIMIT + 1), "prefix": ""},
            {"id": True, "before": "", "prefix": ""},
            {"id": 0, "before": "", "prefix": ""},
            {"id": 2**64, "before": "", "prefix": ""},
        ):
            source = io.BytesIO()
            bridge.send(source, request)
            source.seek(0)
            with self.assertRaises(ValueError, msg=repr(request)):
                bridge.serve(None, source, io.BytesIO())

    def nearby_model(self, calls):
        class Model:
            def predict_words(self, **kwargs):
                calls.append(kwargs)
                if "input_sequence" in kwargs:
                    return SimpleNamespace(top=lambda n: [SimpleNamespace(word="Help")])
                return completions("y")()

        return Model()

    def textslinger_types(self):
        self.scopes = []

        @contextlib.contextmanager
        def cancellation_scope(is_cancelled):
            self.scopes.append(is_cancelled)
            yield

        return {
            "textslinger": SimpleNamespace(
                InputEvent=lambda **kwargs: SimpleNamespace(**kwargs),
                InputChannelConfig=lambda **kwargs: SimpleNamespace(**kwargs)),
            "textslinger.cancellation": SimpleNamespace(cancellation_scope=cancellation_scope),
            "textslinger.exceptions": SimpleNamespace(PredictionCancelled=Cancelled),
        }

    def test_a_nearby_search_past_its_budget_falls_back_to_completion(self):
        calls = []

        class Model:
            def predict_words(self, **kwargs):
                calls.append(kwargs)
                if "input_sequence" in kwargs:
                    raise Cancelled
                return completions("lo")()

        with patch.dict(sys.modules, self.textslinger_types()):
            replies = self.run_requests(Model(), [
                {"id": 1, "before": "", "prefix": "hel", "keys": [["h", 0, 0], ["e", 1, 0], ["l", 2, 0]]}])
        self.assertEqual(replies[1], {"id": 1, "words": ["hello"]})
        self.assertEqual(["input_sequence" in call for call in calls], [True, False])
        # The scope cancels once the budget has passed, and not before.
        is_cancelled = self.scopes[0]
        self.assertFalse(is_cancelled())
        with patch.object(bridge.time, "monotonic", return_value=float("inf")):
            self.assertTrue(is_cancelled())

    def test_a_word_after_punctuation_is_only_completed(self):
        calls = []
        keys = [["h", 0, 0], ["e", 1, 0], ["r", 2, 0], ["l", 3, 0]]
        with patch.dict(sys.modules, self.textslinger_types()):
            replies = self.run_requests(self.nearby_model(calls), [
                {"id": 1, "before": "(", "prefix": "hrl", "keys": keys},
                {"id": 2, "before": "he said '", "prefix": "hrl", "keys": keys},
                {"id": 3, "before": "well-", "prefix": "hrl", "keys": keys}])
        self.assertEqual([r["words"] for r in replies[1:-1]], [["hrly"]] * 3)
        self.assertTrue(all("input_sequence" not in call for call in calls))

    def test_nearby_keys_turn_each_press_into_likely_keys(self):
        calls = []
        keys = [["q", 0, 0], ["w", 1, 0], ["e", 2, 0], ["r", 3, 0], ["t", 4, 0], ["y", 5, 0],
                ["a", 0.25, 1], ["s", 1.25, 1], ["d", 2.25, 1], ["f", 3.25, 1], ["g", 4.25, 1],
                ["h", 5.25, 1], ["j", 6.25, 1], ["k", 7.25, 1], ["l", 8.25, 1], ["p", 9, 0],
                ["u", 6, 0], ["i", 7, 0], ["o", 8, 0], ["n", 5.75, 2], ["b", 4.75, 2]]
        with patch.dict(sys.modules, self.textslinger_types()):
            replies = self.run_requests(self.nearby_model(calls), [
                {"id": 1, "before": "Can you ", "prefix": "HRL", "keys": keys}])
        self.assertEqual(replies[1], {"id": 1, "words": ["help"]})
        call = calls[0]
        # The context is the text before the word; the word is the presses.
        self.assertEqual(call["left_context"], "can you ")
        self.assertEqual(len(call["input_sequence"]), 3)
        press = dict(call["input_sequence"][0].alternatives)
        self.assertEqual(press["h"], 0.0)
        self.assertLess(press["g"], press["h"])
        self.assertIn("y", press)
        self.assertNotIn("q", press)
        self.assertNotIn("p", press)

    def test_short_or_unplaced_prefixes_are_only_completed(self):
        calls = []
        keys = [["h", 0, 0], ["e", 1, 0]]
        with patch.dict(sys.modules, self.textslinger_types()):
            replies = self.run_requests(self.nearby_model(calls), [
                {"id": 1, "before": "", "prefix": "h", "keys": keys},
                {"id": 2, "before": "", "prefix": "hex", "keys": keys},
                {"id": 3, "before": "", "prefix": "he'", "keys": keys},
                {"id": 4, "before": "", "prefix": "he", "keys": []}])
        self.assertEqual([r["words"] for r in replies[1:-1]], [["hy"], ["hexy"], ["he'y"], ["hey"]])
        self.assertTrue(all("input_sequence" not in call for call in calls))

    def test_malformed_keys_are_rejected_before_model_call(self):
        for keys in ({}, [["h", 0]], [["h", 0, 0], ["h", 1, 0]], [["H", 0, 0]], [["1", 0, 0]],
                     [["h", True, 0]], [["h", "0", 0]], [["h", float("nan"), 0]],
                     [["h", float("inf"), 0]], [["'", 0, 0]], [["", 0, 0]],
                     [[c, 0, 0] for c in bridge.LETTERS] + [["a", 0, 0]]):
            source = io.BytesIO()
            bridge.send(source, {"id": 1, "before": "", "prefix": "he", "keys": keys})
            source.seek(0)
            with self.assertRaises(ValueError, msg=repr(keys)):
                bridge.serve(None, source, io.BytesIO())
        # An integer too large for a float.
        for raw in (b'{"id": 1, "before": "", "prefix": "he", "keys": [["h", 1' + b"0" * 400 + b', 0]]}',):
            source = io.BytesIO(struct.pack("<I", len(raw)) + raw)
            with self.assertRaises(ValueError, msg=repr(keys)):
                bridge.serve(None, source, io.BytesIO())

    def test_watchdog_covers_requests_but_not_idle_waiting(self):
        source = io.BytesIO()
        bridge.send(source, {"id": 1, "before": "", "prefix": "2"})
        boundary = source.tell()
        bridge.send(source, {"shutdown": True})
        checks = {0: 0, boundary: 1}
        with patch.object(bridge.threading, "Timer") as timer:
            class CheckedInput(io.BytesIO):
                def read(self, count):
                    if self.tell() in checks and timer.call_count != checks[self.tell()]:
                        raise AssertionError("watchdog started while idle")
                    return super().read(count)
            bridge.serve(None, CheckedInput(source.getvalue()), io.BytesIO())
            self.assertEqual(timer.call_count, 2)
            self.assertEqual(timer.call_args.args[0], bridge.REQUEST_WATCHDOG_SECONDS)


class Runtime(unittest.TestCase):
    def write_runtime(self, directory, **manifest):
        runtime = Path(directory)
        (runtime / "model").mkdir()
        (runtime / "model/m.kenlm").write_bytes(b"model")
        values = {"format_version": 1, "model": "model/m.kenlm", "model_alphabet": "ab "}
        values.update(manifest)
        (runtime / "manifest.json").write_text(json.dumps(values), encoding="utf-8")
        return runtime

    def test_manifest_gives_model_and_alphabet(self):
        with tempfile.TemporaryDirectory() as directory:
            runtime = self.write_runtime(directory)
            path, alphabet = bridge.runtime_model(runtime)
            self.assertEqual(path, runtime / "model/m.kenlm")
            self.assertEqual(alphabet, ("a", "b", " "))

    def test_manifest_must_be_understood_and_stay_inside_the_runtime(self):
        for manifest in ({"format_version": 2}, {"model": "../outside.kenlm"}):
            with tempfile.TemporaryDirectory() as directory:
                runtime = self.write_runtime(directory, **manifest)
                with self.assertRaises(ValueError, msg=repr(manifest)):
                    bridge.runtime_model(runtime)

    def test_missing_model_is_rejected_before_import(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.dict(sys.modules, {"textslinger": None}):
                with self.assertRaises(FileNotFoundError):
                    bridge.load_model(Path(directory) / "missing.kenlm", ("a",))

    def test_loading_passes_the_alphabet_and_warms_up_with_the_serving_settings(self):
        calls = []
        created = []

        def model_class(path, model_alphabet):
            created.append((path, model_alphabet))
            return SimpleNamespace(predict_words=lambda **kwargs: (
                calls.append(kwargs) or SimpleNamespace(top=lambda n: ["ready"])))

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "m.kenlm"
            path.write_bytes(b"model")
            with patch.dict(sys.modules, {"textslinger": SimpleNamespace(NGramLanguageModel=model_class)}):
                bridge.load_model(path, ("a", " "))
        self.assertEqual(created, [(str(path), ("a", " "))])
        self.assertEqual(calls[0]["word_spelling_characters"], bridge.SPELLING)
        self.assertEqual(calls[0]["word_ending_characters"], (" ",))
        self.assertEqual(calls[0]["nbest"], 5)


def fault_bridge(mode):
    """Synthetic peer for the Rust adapter's fault-injection tests."""
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
