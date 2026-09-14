# go_teacher

Analyze Go game records (SGF) move by move with a local KataGo engine and produce a long
Markdown report that an AI teaching agent can read to give stylistic feedback on mistakes.

## Getting started (no Go software installed yet)

Go Teacher needs the KataGo engine and a neural network. The easiest way to get both is to
install **KaTrain**, a free Go client that ships with KataGo built in:

1. Download KaTrain for macOS from <https://github.com/sanderland/katrain/releases> (or run
   `brew install --cask katrain`) and drag it into `/Applications`. Open it once: the first
   start takes a minute while KataGo tunes itself for your GPU.
2. Open the Go Teacher DMG and drag **Go Teacher** into `/Applications`.
3. Launch Go Teacher. It finds KaTrain's engine and network by itself, shows
   "KataGo is ready", and you can upload an SGF file.

If you launch Go Teacher before installing KaTrain, the window shows this guide with a
Retry button instead; nothing else needs relaunching.

That is all. Go Teacher brings its own analysis configuration (500 visits per move, tuned for
Apple Silicon), so nothing needs editing.

### Play against a human-like opponent, and get human-level teaching

KaTrain can play you with KataGo's **human-style network**, which imitates players of a chosen
rank (20 kyu to 9 dan) instead of playing perfect moves. Games against it are far more
instructive than games against full-strength KataGo, and Go Teacher uses the same network to
tell you *how often a player of your level makes the mistake you made* and what a stronger
player would do instead. Setting it up takes two minutes:

1. Download `b18c384nbt-humanv0.bin.gz` from <https://github.com/lightvector/KataGo/releases>
   (attached to release v1.15.0 and later) and save it somewhere permanent, e.g.
   `~/.katago/default_human_model.bin.gz`.
2. In KaTrain: Settings → Engine → "Human-like model", pick that file. Then in New Game choose
   the **Human-like** AI strategy and set its rank in Settings → AI.
3. Go Teacher picks the same file up from KaTrain's settings automatically; the "Human policy
   profile" selector appears in its upload form.

### Using your own KataGo, network or config

Every engine file can be set individually, in the **Engine settings** card of the app (saved to
`~/Library/Application Support/GoTeacher/settings.json`; saving restarts the engine), or on the
command line:

| File | Flag / environment variable | If not set |
|---|---|---|
| KataGo executable | `--katago`, `GO_TEACHER_KATAGO` | KaTrain's configured engine, else the one bundled in KaTrain.app, else `/opt/homebrew/bin/katago` |
| Network | `--model`, `GO_TEACHER_MODEL` | KaTrain's configured network, else KaTrain's bundled one, else `~/.katago/default_model.bin.gz` |
| Analysis config | `--config`, `GO_TEACHER_CONFIG` | the built-in config, written to `~/Library/Application Support/GoTeacher/analysis.cfg` (edit it there, or delete it to restore the default) |
| Human-style network | `--human-model`, `GO_TEACHER_HUMAN_MODEL`; `--no-human-model` to disable | KaTrain's human-like model, else `~/.katago/default_human_model.bin.gz`, else none |

Stronger networks than the one KaTrain bundles are free to download from
<https://katagotraining.org/networks/>; point the Network setting at the downloaded file. Go
Teacher prints where each file came from on startup ("Engine: ...") and shows it in the
Engine settings card. For the fastest setup on Apple Silicon (Homebrew KataGo with the Metal
backend, top network, human network) follow [docs/POWER_USER_KATAGO.md](docs/POWER_USER_KATAGO.md).

## Build from source

Requires the Rust toolchain (`curl https://sh.rustup.rs -sSf | sh`).

```
cargo build --release
# binary: target/release/go_teacher
```

## Use

Start the app (a native window hosting the UI; pass `--browser` to use your web browser instead):

```
./target/release/go_teacher
```

The window opens immediately while KataGo loads the model in the background; a macOS
notification and a sound announce when the engine is ready. Click **Upload SGF file(s)**,
pick one or more `.sgf` files, and watch the analysis live: a board follows the positions
as KataGo finishes them, with candidate moves drawn as circles (colour = quality, size =
visits, number = winrate for the mover), a table of candidates with visit and policy bars,
and a winrate chart you can click to inspect any position. When a job finishes, **view
report** renders the Markdown inside the app; you can also open the `.md`/`.json` in your
default application or reveal them in Finder. Reports are written to `./reports/` on the
command line and `~/Documents/GoTeacher` in the app (change with `--out-dir`).

Closing the window or pressing ⌘Q quits Go Teacher and stops the KataGo engine, even while
the model is still loading.

### Board views

The live board has switchable layers (remembered between runs):

- **candidate moves** (default on): KataGo's searched moves as circles.
- **territory** (default on): ownership shading from KataGo, dark for Black, light for White.
- **policy heat map**: the raw network's move probabilities for the whole board, before search.
- **human policy heat map**: where a player of the chosen rank would play. Requires the
  human-style network (see "Play against a human-like opponent" above) and a **Human policy
  profile** chosen in the upload form, e.g. `rank_5k`, `preaz_1d`, `proyear_2000`. The option
  is hidden when no human network is loaded.

Ownership and policy arrays are stored only in the JSON dump, never in the Markdown report.

### What the report contains for the teaching agent

The Markdown report is written for a language model. Besides per-player statistics it has a
**Teaching candidates** section: the student's most instructive mistakes with facts computed
from the board and the ownership maps (where the points went, whether the played stone was
later captured, groups left in atari, whether KataGo's move answers the opponent's last move,
the network's and the human-style network's probability for the played move) plus a
puzzle-ready stone list, and a list of "only good move" finds worth praising. Only key moves
get a full entry with candidate table and diagram; every other move is one line, which keeps
a full game to a few hundred lines.

The student is the side facing an engine-looking opponent name (KaTrain's "AI (...)"), or
Black; override with the Student selector in the upload form or `--student W`.

### Report files

The **Report files on disk** panel lists every `.md` and `.json` in the output folder with
size and date. JSON dumps (roughly 1–2 MB per game) can be deleted one by one or all at
once with **delete all JSON files**; Markdown reports are never removed by that button. A
JSON dump can also be loaded back into the viewer after a restart.

Headless use, without a browser:

```
./target/release/go_teacher analyze game1.sgf game2.sgf            # config's maxVisits
./target/release/go_teacher analyze game.sgf --visits 200          # faster, weaker
./target/release/go_teacher analyze game.sgf --human-profile rank_5k  # add human policy maps
```

Options (all subcommands): `--katago`, `--model`, `--config`, `--human-model`, `--out-dir`,
or the environment variables `GO_TEACHER_KATAGO`, `GO_TEACHER_MODEL`, `GO_TEACHER_CONFIG`,
`GO_TEACHER_HUMAN_MODEL`, `GO_TEACHER_OUT_DIR`. Without them, engine files come from KaTrain's
settings, then KaTrain's bundle, then `/opt/homebrew/bin/katago` with `~/.katago/default_*`. `--port` (default 8642), `--browser`, `--no-open`, `--log-file`.

Analysis strength comes from `maxVisits` in the analysis config unless you enter a value
in the "Visits per move" box (or pass `--visits`).

## macOS app

```
./packaging/make_app.sh
```

produces `dist/Go Teacher.app` and `dist/GoTeacher-<version>.dmg`. Open the DMG and drag
Go Teacher to Applications (or `cp -R "dist/Go Teacher.app" /Applications/`). The app is
ad-hoc signed, so it runs on this Mac without Gatekeeper prompts; on another Mac,
right-click → Open the first time.

When launched from Finder the app shows its window, starts KataGo, writes reports to
`~/Documents/GoTeacher`, and logs to `~/Library/Logs/GoTeacher.log`. Launching it while it
is already running just opens a window onto the running instance. The engine is found the same way
as on the command line (KaTrain's settings, then its bundle).

## What the report contains

1. Game metadata and the analysis settings used.
2. A "how to read this" section aimed at the AI teacher (coordinates, winrate convention,
   point-loss categories).
3. Summary: accuracy per player, point loss by phase, game-flow table, biggest mistakes
   per player, turning points.
4. Move-by-move: evaluation before/after, point loss, KataGo's rank of the played move,
   the top candidate moves with principal variations, SGF comments, and an ASCII board
   diagram after every mistake of at least 3 points and at regular checkpoints.
5. Final position with KataGo's suggested next moves.
6. A compact one-line-per-move appendix.

Winrates are always reported from Black's perspective. Point loss is computed from the
change in KataGo's score lead between consecutive positions, so it uses the full-visit
search of each position rather than the shallower candidate-move estimate.

## How it works

`go_teacher` starts `katago analysis` once and keeps it running. Each uploaded game becomes
one query with `analyzeTurns` covering every position, so KataGo analyzes positions in
parallel according to `numAnalysisThreads` in the config. Responses stream back per turn,
which drives the progress bar. Cancelling a job sends a `terminate` action to KataGo.

## Layout

- `src/sgf.rs` SGF parser (main line, setup stones, metadata, comments)
- `src/board.rs` board state with captures and ASCII rendering
- `src/katago.rs` KataGo analysis-engine process driver
- `src/analysis.rs` per-move review model and query construction
- `src/report.rs` Markdown report
- `src/server.rs` axum web server, job manager, live-position and report endpoints
- `src/report_html.rs` Markdown → HTML for the in-app report viewer
- `src/window.rs` native window (tao + wry WebView) and menu bar
- `resources/analysis.cfg` the built-in KataGo analysis config
- `static/index.html` upload page and live analysis view
- `packaging/make_app.sh`, `packaging/make_icon.py` macOS bundle and DMG builder
