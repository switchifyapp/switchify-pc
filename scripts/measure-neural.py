"""Measure the ignored Rust integration fixture and its process-tree RSS.

Requires psutil. Uses synthetic context and fake activity/input adapters only.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

import psutil

parser = argparse.ArgumentParser(description=__doc__)
source = parser.add_mutually_exclusive_group(required=True)
source.add_argument('--test-binary', type=Path)
source.add_argument('--build', action='store_true', help='Build and locate the release integration fixture')
parser.add_argument('--model', type=Path, default=os.environ.get('SWITCHIFY_BENCHMARK_MODEL'))
parser.add_argument('--worker', type=Path, default=os.environ.get('SWITCHIFY_BENCHMARK_WORKER'))
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
if args.model is None or args.worker is None:
    parser.error('Explicit model and worker paths or benchmark environment variables are required')
if args.build:
    built = subprocess.run(['cargo', 'test', '--release', '--lib', '--locked',
                            '--manifest-path', 'src-tauri/Cargo.toml',
                            'neural_integration_benchmark', '--no-run', '--message-format=json'],
                           check=True, capture_output=True, text=True)
    artifacts = [json.loads(line) for line in built.stdout.splitlines()]
    binaries = [item['executable'] for item in artifacts
                if item.get('reason') == 'compiler-artifact'
                and item.get('target', {}).get('name') == 'switchify_pc_lib'
                and item.get('executable')]
    if len(binaries) != 1:
        raise RuntimeError('Could not identify the integration test executable')
    args.test_binary = Path(binaries[0])
with tempfile.TemporaryDirectory() as temporary:
    report_path = Path(temporary) / 'report.json'
    env = dict(os.environ, SWITCHIFY_BENCHMARK_MODEL=str(args.model.resolve()),
               SWITCHIFY_BENCHMARK_WORKER=str(args.worker.resolve()),
               SWITCHIFY_NEURAL_REPORT=str(report_path))
    with (Path(temporary) / 'output.txt').open('w') as output:
        process = subprocess.Popen([str(args.test_binary.resolve()), 'neural_integration_benchmark',
                                    '--ignored', '--nocapture', '--test-threads=1'], env=env,
                                   stdout=output, stderr=subprocess.STDOUT)
        tracked = psutil.Process(process.pid)
        peak = 0
        start = time.monotonic()
        while process.poll() is None:
            try:
                rss = 0
                for child in [tracked, *tracked.children(recursive=True)]:
                    try:
                        rss += child.memory_info().rss
                    except (psutil.NoSuchProcess, psutil.AccessDenied):
                        pass
                peak = max(peak, rss)
            except psutil.NoSuchProcess:
                pass
            if time.monotonic() - start > 1200:
                for child in tracked.children(recursive=True):
                    child.kill()
                process.kill()
                process.wait()
                raise RuntimeError('Integration benchmark exceeded 20 minutes')
            time.sleep(0.02)
    if report_path.is_file():
        report = json.loads(report_path.read_bytes())
        report['process_tree_peak_rss_bytes'] = peak
        report['memory_note'] = '20ms sampled sum of test process and child RSS; shared pages may be counted twice and short peaks missed.'
        args.output.write_bytes((json.dumps(report, indent=2) + '\n').encode())
        print(json.dumps(report))
    if process.returncode:
        raise RuntimeError((Path(temporary) / 'output.txt').read_text())
    if not report_path.is_file():
        raise RuntimeError('Integration fixture did not produce a measurement report')
