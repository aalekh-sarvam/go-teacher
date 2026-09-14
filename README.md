# go_teacher

Analyze Go game records (SGF) move by move with a local KataGo engine and produce a long
Markdown report that an AI teaching agent can read to give stylistic feedback on mistakes.

## Setup (one time, about 15 minutes)

Go Teacher analyses games with the **KataGo** engine and shares its engine setup with
**KaTrain**, a free Go program you can also play against. Everyone follows the same steps;
there is no separate "power user" path. When Go Teacher cannot find an engine it shows this
guide inside the app, with a Retry button.

1. **Install KaTrain.** Download it from <https://github.com/sanderland/katrain/releases>
   (or `brew install --cask katrain`), drag it into `/Applications`, open it once. KaTrain
   works right away with a small built-in engine; the following steps make it several times
   faster and much stronger, and Go Teacher will use the same setup.
2. **Install KataGo with the Metal backend**, which uses the Apple GPU and Neural Engine
   (needs [Homebrew](https://brew.sh)):

   ```
   brew install katago
   katago version        # must print "Using Metal backend"
   mkdir -p ~/.katago
   ```

3. **Download the latest network.** On <https://katagotraining.org/networks/> take the newest
   *transformer* network (names start with `b11c768h12nbt` or newer) and save it as
   `~/.katago/default_model.bin.gz`.
4. **Download the human-style network.** Get `b18c384nbt-humanv0.bin.gz` from
   <https://github.com/lightvector/KataGo/releases> (attached to v1.15.0 and later) and save
   it as `~/.katago/default_human_model.bin.gz`. This network imitates players of a chosen
   rank, 20 kyu to 9 dan. In KaTrain it gives you an opponent at your own level (New Game →
   AI strategy **Human-like**, rank in Settings → AI), which is far more instructive than
   losing to full-strength KataGo. In Go Teacher it powers the human policy heat map and the
   "a player of your rank makes this move 30% of the time" facts in the report.
5. **Tell KaTrain.** Settings → Engine: executable `/opt/homebrew/bin/katago`, model
   `~/.katago/default_model.bin.gz`, human-like model `~/.katago/default_human_model.bin.gz`.
   Leave the config field alone unless you have your own analysis config. Save.
6. **Install and launch Go Teacher** from the DMG. It reads KaTrain's engine settings, starts
   the same engine, and the green banner reports the KataGo version, **Metal backend** and the
   network name. If the banner says OpenCL, the engine in use is KaTrain's built-in one: finish
   steps 2 to 5.

### Where the engine files come from

Go Teacher resolves each file separately, in this order:

| File | Flag / environment variable | Then |
|---|---|---|
| KataGo executable | `--katago`, `GO_TEACHER_KATAGO` | Engine settings card → KaTrain's settings → KaTrain's bundled engine → `/opt/homebrew/bin/katago` |
| Network | `--model`, `GO_TEACHER_MODEL` | Engine settings card → KaTrain's settings → KaTrain's bundled network → `~/.katago/default_model.bin.gz` |
| Analysis config | `--config`, `GO_TEACHER_CONFIG` | Engine settings card → a config you set in KaTrain → the built-in config (Metal, 500 visits), written to `~/Library/Application Support/GoTeacher/analysis.cfg` |
| Human-style network | `--human-model`, `GO_TEACHER_HUMAN_MODEL`; `--no-human-model` disables | Engine settings card → KaTrain's human-like model → `~/.katago/default_human_model.bin.gz` → none |

The **Engine settings** card in the app overrides KaTrain for people who do not use it; saving
restarts the engine. The startup line `Engine: ...` and the card show which source was used.

### Sharing the GPU with KaTrain

KaTrain and Go Teacher each start their own KataGo process. A second program cannot attach to
the engine KaTrain already launched (it talks to KataGo over a private pipe), so when both run
with the same network the network is loaded twice and the two searches share the GPU. Go
Teacher detects this and shows a notice; quit KaTrain, or let it finish its game, when you want
full analysis speed.

### Working with results

- **Report viewer**: "view report" renders the Markdown; every "Move N" heading is a link that
  opens that position in the live board.
- **Variation exploration**: with "follow latest" off, click an empty point on the live board
  to ask KataGo what happens after that move; keep clicking to go deeper, "back" to retract.
- **Lesson bundle**: the report page's "lesson bundle" link zips the report, the JSON dump and,
  if the lesson skill folder is set in Engine settings, the skill's parsed JSON and SKILL.md,
  then reveals the zip in Finder for upload to your teaching agent.
- **Resume**: a cancelled or failed analysis keeps the positions it finished; "resume" completes
  the rest. If KataGo crashes mid-game, Go Teacher restarts it and resumes the job by itself.
- **Persistence**: finished analyses reappear after a restart (loaded from their JSON dumps on
  demand; idle ones are dropped from memory after ten minutes).
- **Watch folder**: set a folder in Engine settings (KaTrain's save folder, an iPad sync folder)
  and new `.sgf` files saved there are analysed automatically, with an optional human profile.
- **Drag and drop / Finder**: drop SGF files on the window or the page, or double-click an SGF in
  Finder and choose Go Teacher.
- **Engine speed test**: "Test engine speed" in Engine settings runs `katago benchmark` for
  20 seconds and shows the visits per second.

### Batches

Upload several SGF files at once, or keep uploading while a game is being analysed: games run
one at a time in upload order, queued ones show their position, and **cancel all** stops the
batch. On the command line, directories expand to every `.sgf` inside them:

```
go_teacher analyze ~/Documents/Go_games/            # every game in the folder, in order
```

## Build from source

Requires the Rust toolchain (`curl https://sh.rustup.rs -sSf | sh`). `cargo test` runs the unit
tests and an end-to-end test against a fake engine, so no GPU or model is needed.

For a distributable DMG, sign and notarize with a Developer ID by setting
`CODESIGN_IDENTITY="Developer ID Application: ..."` and `NOTARY_PROFILE=<keychain profile>`
before running `./packaging/make_app.sh` (one-time setup: `xcrun notarytool store-credentials`).

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
  human-style network (setup step 4) and a **Human policy profile** chosen in the upload form,
  e.g. `rank_5k`, `preaz_1d`, `proyear_2000`. The option is hidden when no human network is
  loaded.

Ownership and policy arrays are stored only in the JSON dump, never in the Markdown report.

### What the report contains for the teaching agent

The Markdown report is written for a language model and carries a `Report format: N` line
that the lesson skill's parser checks. Besides per-player statistics it has a
**Teaching candidates** section: the student's most instructive mistakes with facts computed
from the board and the ownership maps (where the points went, whether the played stone was
later captured, groups left in atari, whether KataGo's move answers the opponent's last move,
the network's and the human-style network's probability for the played move) plus a
puzzle-ready stone list, a rule-based **theme**, the opponent's strongest punishment after the
played move ("what the move allows"), KataGo's line after the better move, and the chain of
moves played in the same area before and after. A **Game arc facts** section gives per-phase
numbers and every life-and-death change detected from the ownership maps, as material for a
phase-by-phase narrative; a list of "only good move" finds is included for praise. On 13x13
and larger boards an **Opening patterns** section names how each corner was played and the
first move KataGo disliked there. When the record has clock data, thinking time per move and a
fast-versus-slow loss table appear. Once a student has two or more earlier analysed games, a
**Compared with the student's earlier games** section shows trends (kept in `progress.json`
in the reports folder). A second, stronger human profile ("target") can be chosen at upload to
show what a player two stones stronger would do. Only key moves
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
./target/release/go_teacher analyze ~/Documents/Go_games/          # a whole folder
./target/release/go_teacher analyze game.sgf --visits 200          # faster, weaker
./target/release/go_teacher analyze game.sgf --human-profile rank_5k  # add human policy maps
./target/release/go_teacher analyze game.sgf --two-pass --human-profile rank_10k --human-profile-target rank_3k
```

Options (all subcommands): `--katago`, `--model`, `--config`, `--human-model`, `--out-dir`,
or the environment variables `GO_TEACHER_KATAGO`, `GO_TEACHER_MODEL`, `GO_TEACHER_CONFIG`,
`GO_TEACHER_HUMAN_MODEL`, `GO_TEACHER_OUT_DIR`. Without them, engine files come from KaTrain's
settings, then KaTrain's bundle, then `/opt/homebrew/bin/katago` with `~/.katago/default_*`. `--port` (default 8642), `--browser`, `--no-open`, `--log-file`.

Analysis strength comes from `maxVisits` in the analysis config unless you enter a value
in the "Visits per move" box (or pass `--visits`). **Two-pass** mode (on by default in the
app; `--two-pass` on the command line) first analyses every position quickly (150 visits, or
the visits you enter) and then re-analyses the key positions deeply (1000 visits, or
`--deep-visits`): the student's teaching candidates before and after, big swings in the
undecided part of the game, the opponent's biggest mistakes and the final position. This
sharpens the numbers where the lesson is at about the same total time as a flat 500.

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
- `src/teaching.rs` teaching candidates, themes, chains, status changes, phase facts
- `src/opening.rs` corner opening patterns
- `src/progress.rs` cross-game progress records
- `tests/fake_katago.py` a stand-in engine for `cargo test`
- `static/index.html` upload page and live analysis view
- `packaging/make_app.sh`, `packaging/make_icon.py` macOS bundle and DMG builder
