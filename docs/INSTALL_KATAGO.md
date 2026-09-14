# Installing KataGo on a Mac (Metal) and pointing Go Teacher at it

Go Teacher does not ship an engine. It drives a locally installed KataGo through its JSON
analysis protocol. This guide sets up KataGo with the Metal backend (Apple GPU plus the
Neural Engine) on Apple Silicon and tells Go Teacher where everything is.

## 1. Install KataGo

Homebrew's bottle is built with the Metal backend on Apple Silicon:

```
brew install katago
katago version          # should print "Using Metal backend"
```

The binary lands at `/opt/homebrew/bin/katago`, which is Go Teacher's default.

## 2. Download the networks

Create a folder for engine files, for example `~/.katago`:

```
mkdir -p ~/.katago
```

**Main network.** Get the current strongest network from <https://katagotraining.org/networks/>
(the transformer nets, names starting with `b11c768h12nbt`, are the strongest and fast on
Apple Silicon; the `kata1-b28c512nbt` / `b40c768nbt` convolutional nets also work but are
slower). Save it as:

```
~/.katago/default_model.bin.gz
```

**Human-style network (optional, needed for the human policy heat maps and "how often does a
5k play this" numbers).** Download `b18c384nbt-humanv0.bin.gz` from the KataGo releases page,
<https://github.com/lightvector/KataGo/releases> (attached to the v1.15.0 release and later),
and save it as:

```
~/.katago/default_human_model.bin.gz
```

If this file is absent Go Teacher starts without it; pass `--no-human-model` to skip it
explicitly.

## 3. Create the analysis config

Start from the example that ships with KataGo:

```
cp "$(brew --prefix katago)/share/katago/configs/analysis_example.cfg" ~/.katago/default_analysis.cfg
```

Then edit `~/.katago/default_analysis.cfg`. The settings that matter:

```
logDir = /Users/<you>/.katago/analysis_logs   # one log file per run
maxVisits = 500                               # analysis strength; 500 is a good review default
numAnalysisThreads = 2                        # positions analysed in parallel
numSearchThreadsPerAnalysisThread = 4
nnMaxBatchSize = 8
nnCacheSizePowerOfTwo = 23

# Metal backend: split network evaluations between the GPU and the Neural Engine
numNNServerThreadsPerModel = 4
metalDeviceToUseThread0 = 0       # 0 = GPU (MPSGraph)
metalDeviceToUseThread1 = 0
metalDeviceToUseThread2 = 100     # 100 = Neural Engine (CoreML, FP16)
metalDeviceToUseThread3 = 100
```

GPU-only is the simplest alternative if the Neural Engine path gives trouble:

```
numNNServerThreadsPerModel = 1
metalDeviceToUseThread0 = 0
```

Go Teacher overrides `reportAnalysisWinratesAs` to `BLACK` itself, so leave that as is.

Check that everything loads and see your speed:

```
katago benchmark -model ~/.katago/default_model.bin.gz -config ~/.katago/default_analysis.cfg
```

Expect roughly 5 seconds per position at 500 visits on an M-series laptop, so a full
19x19 game takes 15–20 minutes. Other engines using the GPU at the same time (KaTrain, for
example) slow this down noticeably.

## 4. Point Go Teacher at the files

With the paths above nothing needs configuring: the defaults are

| Setting | Default |
|---|---|
| KataGo executable | `/opt/homebrew/bin/katago` |
| Main network | `~/.katago/default_model.bin.gz` |
| Analysis config | `~/.katago/default_analysis.cfg` |
| Human network | `~/.katago/default_human_model.bin.gz` (if present) |

To use other locations, either pass flags:

```
go_teacher --katago /path/to/katago --model /path/to/net.bin.gz --config /path/to/analysis.cfg \
           --human-model /path/to/b18c384nbt-humanv0.bin.gz
```

or set environment variables, which also work for the app bundle launched from Finder when
set in `launchctl` or your shell profile:

```
export GO_TEACHER_KATAGO=/path/to/katago
export GO_TEACHER_MODEL=/path/to/net.bin.gz
export GO_TEACHER_CONFIG=/path/to/analysis.cfg
export GO_TEACHER_HUMAN_MODEL=/path/to/b18c384nbt-humanv0.bin.gz
```

Note that the defaults are compiled into the binary from the paths on the build machine;
if you build on a different account, edit the `DEFAULT_*` constants at the top of
`src/main.rs` or rely on the environment variables.

## 5. Verify from Go Teacher

```
go_teacher analyze some_game.sgf --visits 30
```

should print "KataGo ready", analyse every position, and write a report into `./reports/`.
If the engine fails to start, run the `katago benchmark` command from step 3 by hand: its
error messages are far more specific than Go Teacher's.

## Troubleshooting

- **"Using Metal backend" not printed**: the Homebrew bottle for Intel Macs uses the Eigen
  (CPU) backend, which is far too slow for review. On Intel, build KataGo from source with the
  OpenCL backend instead.
- **Slow first start**: the Metal backend compiles the network on first use and CoreML
  compiles the Neural Engine model; the first run after a new network can take a minute or two.
- **Model version errors**: transformer networks need KataGo 1.17 or newer; `brew upgrade katago`.
