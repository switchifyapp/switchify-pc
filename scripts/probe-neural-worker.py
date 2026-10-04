"""Text-free packaged-worker diagnostic with a fixed synthetic query.

The five-second diagnostic bound measures slow workers without changing the
application's two-second inference deadline. No desktop input is injected.
"""
import json
import os
from pathlib import Path
import queue
import struct
import subprocess
import threading
import time


def read_frame(stream, replies):
    try:
        header = stream.read(4)
        if len(header) != 4:
            raise ValueError()
        length = struct.unpack('<I', header)[0]
        if not 0 < length <= 65536:
            raise ValueError()
        replies.put(json.loads(stream.read(length)))
    except (ValueError, OSError):
        replies.put(None)


def receive(process, timeout):
    replies = queue.Queue(maxsize=1)
    reader = threading.Thread(target=read_frame, args=(process.stdout, replies), daemon=True)
    reader.start()
    try:
        return replies.get(timeout=timeout)
    except queue.Empty:
        return None


def main():
    worker = Path(os.environ['SWITCHIFY_BENCHMARK_WORKER'])
    bundle = Path(os.environ['SWITCHIFY_BENCHMARK_MODEL']).parent.parent / 'prediction-neural'
    report = {'ready': False, 'generated': False}
    start = time.monotonic()
    process = subprocess.Popen([str(worker), str(bundle)], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                               env=dict(os.environ, RAYON_NUM_THREADS='4',
                                        TOKENIZERS_PARALLELISM='false'))
    try:
        ready = receive(process, 30)
        report['ready_ms'] = (time.monotonic() - start) * 1000
        report['ready'] = isinstance(ready, dict) and ready.get('Ready', {}).get('version') == 2
        if report['ready']:
            frame = json.dumps({'Generate': {'id': 1, 'session': 0,
                                'before': 'please send the', 'prefix': '',
                                'exclude': ['receipt', 'order', 'tickets'], 'limit': 3}}).encode()
            start = time.monotonic()
            process.stdin.write(struct.pack('<I', len(frame)) + frame)
            process.stdin.flush()
            ranked = receive(process, 5)
            report['query_ms'] = (time.monotonic() - start) * 1000
            report['generated'] = isinstance(ranked, dict) and 'Generated' in ranked
        report['exit_before_cleanup'] = process.poll()
    finally:
        process.kill()
        process.wait()
        process.stdin.close()
        process.stdout.close()
    print(json.dumps(report))


if __name__ == '__main__':
    main()
