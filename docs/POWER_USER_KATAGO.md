# Power users: a faster, stronger KataGo for Go Teacher

Out of the box Go Teacher uses whatever engine KaTrain uses, which for a fresh KaTrain install
is KataGo with the OpenCL backend and a small network. That is fine for reviews but leaves a
lot on the table on Apple Silicon. This guide installs KataGo with the **Metal backend**
(Apple GPU plus the Neural Engine), the strongest current network, and the human-style
network that powers the human policy heat maps. Expect several times the speed at the same
visits, or the same speed with a much stronger network.

Once done, either point **KaTrain** at the new files in its Engine settings (Go Teacher reads
KaTrain's settings, so both programs switch together) or enter them in Go Teacher's own
Engine settings card, which takes precedence over KaTrain.

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

Either enter the paths in Go Teacher's **Engine settings** card (saved to
`~/Library/Application Support/GoTeacher/settings.json`; saving restarts the engine), or in KaTrain's Settings → Engine
(saved to `~/.katrain/config.json`, which Go Teacher also reads). Precedence, per file:

| Priority | Source |
|---|---|
| 1 | flags or `GO_TEACHER_*` environment variables |
| 2 | Go Teacher's `settings.json` |
| 3 | KaTrain's settings (`~/.katrain/config.json`, section `engine`) — engine, network, human network only |
| 4 | the engine and network bundled inside KaTrain.app |
| 5 | `/opt/homebrew/bin/katago`, `~/.katago/default_model.bin.gz`, `~/.katago/default_human_model.bin.gz` |

The analysis config never comes from KaTrain: without a setting Go Teacher uses its built-in
config (the one in this guide), written to `~/Library/Application Support/GoTeacher/analysis.cfg`.
To use other locations from the command line, either pass flags:

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

Go Teacher prints which setup it chose on startup ("Engine: ...") and the app's log
(`~/Library/Logs/GoTeacher.log`) records the exact paths.

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
