# TextSlinger local prediction spike

Issue: [#1417](https://github.com/switchifyapp/switchify-pc/issues/1417).

## Finding

Real TextSlinger n-gram predictions can pass through Switchify's Windows Rust
prediction engine and produce the expected selection operation. The experiment
passed with both native Windows Python and a Python process in Ubuntu WSL,
using private inherited pipes. It opens no
network listener and sends no prediction requests to an external service.

This is a test-only feasibility experiment, not a selectable app backend. It
does not exercise a live overlay, inject desktop input, install WSL for users,
or establish that an installer can distribute the native runtime. Production app code is
unchanged apart from including the experiment under `cfg(test)`.

## What was exercised

The ignored tests construct the real `worker::Engine` with a TextSlinger
implementation of `model::Predict`. Only foreground/activity callbacks are
faked. Context extraction, the database boundary, suggestion slots, casing,
revision/token checks, and selection responses use the existing Switchify code.
Selection returns an insertion operation for assertion; no operating-system
input adapter is invoked.

- `I would like a co` produces `couple`; selection appends `uple `.
- `I would like a cup of ` produces `coffee`; selection appends `coffee `.
- `please open the ` produces `door`; selection appends `door `.
- `I would like a cup of CO` displays `COFFEE`; selection appends `FFEE `.
- Backspacing recomputes results. Old tokens and a selection after a foreground
  change cannot insert text. A suggestion cannot be accepted twice.
- Backend shutdown results in `Unavailable` and empty suggestions.

Only five word slots are populated. Phrase slots remain empty. The adapter
supports English ASCII literal-prefix completion, not nearby-key corrections,
abbreviations, noisy touch input, personal learning, or multilingual prediction.
It does not use TextSlinger's edit operation; Switchify retains ownership of
the accepted prefix and constructs its own safe insertion.

## Local measurements, 13 November 2026

One run per route, Windows x64 Rust test executable on this development machine.
These are observations, not performance guarantees or quality results.

| Measurement | Observed |
| --- | --- |
| Backend startup, including WSL/Python imports/model load | 10,917 ms |
| Four warm query round trips through Switchify's engine | 75, 84, 86, 59 ms |
| Real-model integration tests | 2 passed |
| Dependency-free Python transport tests | 3 passed |

The native Windows run also passed both real-model integration tests. Its backend
startup was 11,560 ms and four warm query round trips were 58, 60, 76, and 44 ms.
The same three Python transport tests passed natively. No TextSlinger source
changes were required. The native virtual environment occupies about 815 MB
before adding the 80 MB model; it includes build tools and is not a minimal
distribution size. Both routes still incur heavyweight Python imports even for
this n-gram backend because the package imports transformer backends eagerly.

The ARPA model is 79,923,990 bytes, downloaded as about 21 MiB compressed.
TextSlinger checkout: `af62a0790216983003e78eb70e34916b26157aac`, version `0.4.0`.
Model: the project's `lm_dec19_char_tiny_12gram.arpa` from
`https://osf.io/download/mz3db/`, gzip compressed.
Decompressed SHA-256:
`fa8dd62d09d9306d848cd4cd2c20614e19e9aaed3242df27b6d5e12725d3fb4e`.

The experiment used Python 3.10.22, CPU PyTorch 2.14.1, NumPy 1.26.4,
Transformers 5.20.2, PEFT 0.14.0, KenLM 0.2.0 built with `MAX_ORDER=12`,
and CMake 3.31.10. Only dependencies needed to import and run this backend
were installed. The library was installed editable with `--no-deps`; this is
not a validation of its full declared dependency set or its other backends.

## Reproduce

Use the cloned TextSlinger repository and a separate Python 3.10 WSL environment.
The upstream README describes the supported Linux setup. For the smaller spike
environment, with `uv` and C++ build tools already installed in WSL:

```bash
uv venv --python 3.10 ~/.venvs/textslinger
source ~/.venvs/textslinger/bin/activate
uv pip install 'torch==2.14.1' --index-url https://download.pytorch.org/whl/cpu
uv pip install 'numpy==1.26.4' 'transformers==5.20.2' 'peft==0.14.0' regex \
  'cmake==3.31.10' 'setuptools>=77' wheel
MAX_ORDER=12 uv pip install --no-build-isolation --no-binary kenlm 'kenlm==0.2.0'
uv pip install --no-deps -e /path/to/textslinger
curl -L --fail https://osf.io/download/mz3db/ -o /tmp/textslinger.arpa.gz
gzip -dc /tmp/textslinger.arpa.gz > /path/to/textslinger/assets/lm_dec19_char_tiny_12gram.arpa
sha256sum /path/to/textslinger/assets/lm_dec19_char_tiny_12gram.arpa
```

From the Switchify repository in PowerShell, substitute the Linux paths:

```powershell
./scripts/Test-TextSlingerSpike.ps1 `
  -Python /home/YOUR_USER/.venvs/textslinger/bin/python `
  -Model /path/to/textslinger/assets/lm_dec19_char_tiny_12gram.arpa
```

The launcher translates the bridge's Windows checkout path with `wslpath`.
It restores the previous environment variable afterward. The tests are ignored
by default, require no baseline Switchify model, and use only fixed synthetic
text. To run directly on Linux/macOS or with a native Python installation, set
`SWITCHIFY_TEXTSLINGER_COMMAND` to a JSON argv array of the Python executable,
bridge path, `--model`, and model path, then run the same Cargo test filter.
The direct Linux/macOS launch routes have not been exercised here. The native
Windows route below has been exercised.

### Native Windows route

Native Windows succeeded despite the upstream README recommending WSL. The
default KenLM pip source build first failed with CMake 4's removed old-policy
compatibility. Pinning CMake 3 did not support the installed Visual Studio 2026.
Using CMake 4.4.4, the Visual Studio 18 2026 generator, and the compatibility
environment variable below built KenLM 0.2.0 successfully. The native Python
extension loaded the real model and reported order 12.

With `uv` and Visual Studio 2026 C++ tools installed, create a Python 3.10
environment and run in PowerShell, substituting your paths:

```powershell
uv venv --python 3.10 C:/path/to/textslinger-env
$python = 'C:/path/to/textslinger-env/Scripts/python.exe'
uv pip install --python $python 'cmake==4.4.4' 'setuptools>=77' wheel
$env:PATH = 'C:/path/to/textslinger-env/Scripts;' + $env:PATH
$env:MAX_ORDER = '12'
$env:CMAKE_GENERATOR = 'Visual Studio 18 2026'
$env:CMAKE_POLICY_VERSION_MINIMUM = '3.5'
uv --no-cache pip install --python $python --no-build-isolation --no-binary kenlm 'kenlm==0.2.0'
Remove-Item Env:MAX_ORDER, Env:CMAKE_GENERATOR, Env:CMAKE_POLICY_VERSION_MINIMUM
uv pip install --python $python 'torch==2.14.1' --index-url https://download.pytorch.org/whl/cpu
uv pip install --python $python 'numpy==1.26.4' 'transformers==5.20.2' 'peft==0.14.0' regex
uv pip install --python $python --no-deps -e C:/path/to/textslinger
./scripts/Test-TextSlingerSpike.ps1 -Native -Python $python `
  -Model C:/path/to/textslinger/assets/lm_dec19_char_tiny_12gram.arpa
```

Use the same verified decompressed model as the WSL experiment. `--no-cache`
avoids reusing a failed source-build directory with the old CMake generator.
Different installed Visual Studio versions need their matching generator. These
are developer build instructions, not steps to require of Switchify users.
Only this x64 configuration has been tested. A product package should distribute
verified prebuilt runtime components so users do not need compilers or pip.

Dependency-free Python transport checks:

```bash
python scripts/textslinger-spike-test.py
```

The included Rust test file also needs an explicit formatting check because
`rustfmt` does not traverse `include!` files:

```bash
rustfmt --edition 2021 --check src-tauri/src/prediction/textslinger_spike_tests.rs
```

## Setup and product implications

The native Windows route is the first packaging candidate: this experiment
demonstrates WSL is not necessary for the n-gram integration on this machine.
It still needs a reproducible prebuilt runtime, dependency trimming or lazy
backend imports, model/runtime licence review, signed helper packaging, startup
integration, updates, repair, and clean uninstall. None was implemented here.

The WSL route also works for this backend. A product installer using it would
still need an explicit optional component download, WSL availability/admin and
restart handling, a dedicated managed Linux environment, verified runtime and
model artifacts, licence notices, updates, repair, and uninstall ownership.
No installer changes were attempted in this spike.

Startup must be asynchronous and complete before enabling the backend. The
experiment allows 90 seconds for startup and 1,200 ms for a prediction reply.
Timeout or malformed output makes its adapter unusable so a late response
cannot answer another request. Production should handle this without blocking
keyboard interaction and decide whether to retain the existing predictor as
a fallback.

The bridge uses Switchify's existing 16 KiB framed transport limit. It limits
context length, validates returned ASCII words, discards child diagnostics, and
logs only fixed fixture timings. It holds the model in memory, without learning
or saving typed context. It shuts down cooperatively on EOF or a shutdown
request, with a 30-second watchdog for idle/blocked requests. Killing `wsl.exe`
alone is not a proven process-tree cleanup strategy. Production needs tested
supervision for app crashes, timeouts, suspend/resume, and uninstallation.

macOS has an upstream-supported setup, but no macOS app integration or packaging
was tested. Windows ARM64 and machines without WSL were not tested. LLM backend
latency and memory use remain separate questions.

Recommendation: take the native Windows route into a separate packaging spike
before implementing the optional app backend. This spike establishes the existing prediction
engine boundary can accept real TextSlinger words; it does not justify replacing
the shipped predictor or claiming an improvement in prediction quality.
