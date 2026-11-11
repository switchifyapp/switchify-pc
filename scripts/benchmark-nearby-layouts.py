"""Run only synthetic prediction queries; never starts the app or injects input."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip():
        raise RuntimeError('Commit source changes before measuring')
    build = subprocess.run(['cargo', 'test', '--manifest-path', 'src-tauri/Cargo.toml', '--lib',
                            '--release', '--locked', '--no-run', '--message-format=json'],
                           cwd=ROOT, capture_output=True, text=True, encoding='utf-8', check=True)
    executables = []
    for line in build.stdout.splitlines():
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if (message.get('reason') == 'compiler-artifact' and message.get('executable')
                and message['target']['name'] == 'switchify_pc_lib'
                and message['profile']['test']):
            executables.append(Path(message['executable']))
    if len(executables) != 1 or not executables[0].is_file():
        raise RuntimeError('Cargo did not identify one built library test executable')
    executable = executables[0]
    with tempfile.TemporaryDirectory() as directory:
        report = Path(directory) / 'report.json'
        env = {**os.environ, 'SWITCHIFY_NEARBY_REPORT': str(report)}
        subprocess.run([str(executable), 'prediction::nearby::tests::layout_benchmark', '--exact',
                        '--ignored', '--nocapture', '--test-threads=1'], cwd=ROOT, env=env, check=True)
        result = json.loads(report.read_text(encoding='utf-8'))
    assert result['protocol'] == 'pc-nearby-v1' and len(result['rows']) == 27
    result.update(measurement_commit=head, executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
                  rustc=subprocess.check_output(['rustc', '--version'], text=True).strip(),
                  platform=platform.platform(), logical_cpus=os.cpu_count(), complete=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes((json.dumps(result, indent=2) + '\n').encode('utf-8'))
    print(args.output)


if __name__ == '__main__':
    main()
