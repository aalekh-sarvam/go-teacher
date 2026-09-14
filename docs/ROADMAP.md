# Go Teacher — improvement plan

**Status (2026-09-14):** done — A1 two-pass, A2 refutation lines (from the next position's search, no extra query), A3 themes, A4 group status changes, B1 chart, C4 format version, plus per-phase arc facts and same-area move chains for the skill's game-arc narrative. Open — A5–A8, B2–B9, C1–C3, C5–C6. C7 (thread tuning) dropped: benchmarking already done by the user.

Ideas gathered after the first working version (September 2026), written so each one can be
picked up later without re-deriving the reasoning. Items are grouped by what they buy, and each
has: why, what to build, where in the code, how to verify, and a rough size
(S = an hour or two, M = half a day, L = a day or more). Priorities are at the end.

Conventions used below: "report" is the Markdown written by `src/report.rs`; "live view" is the
board panel in `static/index.html`; "candidate" is a teaching candidate from `src/teaching.rs`;
"the skill" is `skill_indus/go-game-teacher`.

---

## A. Analysis quality and teaching value

### A1. Two-pass analysis (M)

**Why.** A flat 500 visits spends most of the budget on moves nobody will discuss. Analysing
everything cheaply and re-analysing only the interesting positions deeply gives sharper numbers
where the lesson is, in the same wall time.

**Build.**
1. Pass 1: current query with `maxVisits` = `pass1_visits` (default 100).
2. Select positions to deepen: the student's teaching candidates (before and after position, so
   the point loss is computed from two deep searches), the turning points, the opponent's top
   mistakes, and the final position.
3. Pass 2: one query per selected turn (or one query with `analyzeTurns` listing them) with
   `maxVisits` = `pass2_visits` (default 1500). Replace those `TurnEval`s.
4. Recompute reviews, teaching candidates and praise from the merged turns.
5. Report both settings in the "Analysis strength" row and mark deepened moves in the
   move-by-move section ("re-analysed at 1500 visits").

**Where.** `analysis.rs::analyze_game` (split into `run_pass` + selection), `AnalysisOptions`
gains `pass1_visits`, `pass2_visits`, `two_pass: bool`; the upload form gets a "Deep re-analysis"
checkbox; progress reporting must count both passes (`server.rs` progress closure, live view
"done / total").

**Verify.** Same game analysed flat-500 and two-pass; wall time within 20%; point losses of the
teaching candidates change by less than the flat-500 vs flat-1500 difference.

### A2. Refutation lines (M)

**Why.** The PV shows how KataGo would have avoided the mistake. A teacher explains a mistake by
showing what it *allows*: the opponent's best punishment after the played move. The lesson
agent currently has to guess this.

**Build.** For each teaching candidate, run one extra query on the position *after* the played
move (moves[..=i]) asking for the opponent's best line: take the top candidate's PV from that
search (already the opponent to move) and store it as `refutation: Vec<String>` with its score.
Also store `if_answered_correctly`: the PV after the best move (already available as the best
alternative's PV). Render both in the candidate block: "If Black had played D4: ... ; after the
actual G4, White's strongest reply is F2 ... leading to W+4.4".

**Where.** `teaching.rs::build` gains async access to the engine (or a second phase in
`analysis.rs` after selection, passing results into the candidates); `report.rs` candidate
block; JSON field; the skill's parser (`parse_teaching`) and `SKILL.md` step 5 ("walk the
refutation").

**Verify.** Refutation's first move equals the top candidate of the after-position search;
appears in the report and in `parsed.json`.

### A3. Computed mistake themes (S)

**Why.** The hints carry the signals; a small model classifies far better from a label.

**Build.** `TeachingCandidate.theme: Theme` with variants Reading, Shape, Direction, Tenuki,
OverDefence, Endgame, LifeAndDeath, Unknown, assigned by rules in this order:
1. captured_at.is_some() or atari list mentions the mover's stones → Reading
2. ownership flip of a whole group (see A4) → LifeAndDeath
3. best_answers_last && !played_answers_last → Tenuki
4. played_answers_last && !best_answers_last → OverDefence
5. distance_to_best <= 2 → Shape
6. phase == endgame && loss < 4 → Endgame
7. distance_to_best >= 4 → Direction
8. else Unknown
Add a "Theme" column to the teaching table and a `theme` field in the skill parser; the skill
uses it as the default and may override with a stated reason.

**Where.** `teaching.rs`, `report.rs`, `parse_review.py`, `go-teaching-concepts.md` (replace the
hint→theme table with "the report supplies a theme").

**Verify.** Unit tests on synthetic candidates for each rule.

### A4. Group life-and-death status from ownership (M)

**Why.** "Your 7-stone group at C3 went from alive to dead on move 41" is the most concrete
teaching statement available and is fully derivable from data already collected.

**Build.** For each move i: take groups on the board after the move (`Board::groups`), compute
mean ownership per group from `turns[i].ownership` and `turns[i+1].ownership` (mover
perspective); a group whose mean crosses from > +0.5 to < -0.5 (or the reverse) is a status
change. Record `{group_anchor, stones, from, to}` on the review and as a hint on the candidate.
Summary section "Groups that changed status" listing all such events per player.

**Where.** `teaching.rs` (new fn `group_status_changes`), `analysis.rs::MoveReview` (new field),
`report.rs` (summary + candidate hint), JSON, parser.

**Verify.** In the KaTrain 9x9 game, move 33 C5 (captured at 38) should show the C5 group
flipping.

### A5. Cross-game progress (L)

**Why.** One-off reviews become a curriculum when the student can see trends.

**Build.** A `progress.json` in the reports folder updated after each analysis: per game
(date, opponent, result, moves, mean loss per phase, category counts, theme counts, top-3 match).
New UI card "Progress" with charts: mean loss per game over time, theme histogram, phase
breakdown trend. New report section "Compared with your last N games" (only when N ≥ 3).
Student identity: use the student name from the SGF or a name entered once in settings.

**Where.** new `src/progress.rs`, `server.rs` (update after finish, `/api/progress`),
`index.html`, `report.rs`.

**Verify.** Analyse three games; the progress card shows three points; the fourth report has the
comparison section.

### A6. Joseki / fuseki recognition (L)

**Why.** Direction-of-play lessons need to know whether the opening was standard.

**Build.** Small built-in database (a few hundred common corner sequences as move lists,
normalised by corner symmetry and colour). For the first ~20 moves per corner, find the longest
matching prefix; report the first deviation with the standard continuation. 19x19 only.

**Where.** new `src/joseki.rs` + `resources/joseki.json`; report "Opening" subsection; hint on
candidates within the deviation move.

**Verify.** A game with a known 3-3 invasion sequence matches; a deviation is reported at the
correct move.

### A7. Move times from SGF clocks (S)

**Why.** A blunder played in two seconds is a different lesson from one played after two
minutes.

**Build.** Parse `BL`/`WL` (time left) per move in `sgf.rs`; derive time spent = previous
remaining − current remaining (per colour, ignoring byo-yomi resets when the value increases).
Add "Time spent" to full move entries and a hint "played in 2 s" on candidates when below the
game's 25th percentile. Summary: mean loss for fast vs slow moves per player.

**Where.** `sgf.rs::Move` (`time_left: Option<f64>`), `teaching.rs`, `report.rs`, parser.

**Verify.** An OGS SGF with BL/WL renders time columns; a file without them shows nothing.

### A8. Human policy at two ranks (S)

**Why.** "You played what a 10k plays; a 7k would already play X" is a better nudge than a
comparison with KataGo.

**Build.** Optional second profile (`human_profile_target`). KataGo takes one profile per query,
so run the game twice? No: the human policy is one cheap network call per position; instead
issue a second lightweight query (`maxVisits: 1`, `includePolicy: true`,
`overrideSettings.humanSLProfile = target`) over all turns and store `human_policy_target`.
Report: "Human policy (10k / 7k)" columns and hint text with both.

**Where.** `analysis.rs` (second query after the main one), `TurnEval`, `MoveReview`,
`report.rs`, upload form (second dropdown, default "two stones stronger"), parser.

**Verify.** Turn 20 of the KaTrain game shows different top moves for rank_20k and rank_15k.

---

## B. The app

### B1. Score-lead line and per-move loss bars in the live chart (S)

**Why.** The winrate line saturates at 0/100 in beginner games and looks broken. Score lead
moves in the ±10 range and per-move loss is the signal that matters.

**Build.** In `drawChart`: second y-axis for score lead (line, clamped ±30), loss bars per move
coloured by player (Black dark, White light), winrate line kept but drawn thinner. Toggle in the
view options. Uses `series` (already carries score_lead) plus a new `losses` array in
`/api/jobs/:id/live` built from `live` turns (loss = sign × Δscore).

**Verify.** KaTrain 9x9 game: bars of ~10 points alternate; the score line stays within ±10.

### B2. Report viewer linked to the live board (M)

**Why.** The Markdown viewer is static text; the position data is already in memory.

**Build.** In `report_html.rs`, wrap "Move N" headings and appendix lines in links
`/?job=ID&turn=N`; the main page reads the query string and selects that job/turn (follow off).
Alternatively embed the live board in the report page via an iframe of the main page in a
"viewer" mode. Prefer the first: less code.

**Verify.** Clicking "Move 15" in the report opens the main page showing position 15.

### B3. Variation exploration (L)

**Why.** Turns the viewer into a study tool without a full game editor.

**Build.** Clicking a candidate circle (or an empty point) on the live board posts
`/api/jobs/:id/explore` with `{turn, moves: [...]}`; the server runs a single-position query
(`maxVisits` from the config, `includeOwnership`) on the game moves up to `turn` plus the
variation, and returns a `LiveTurn`-shaped payload; the board shows it with a breadcrumb
("after 15: G4 → D4 → C4") and a "back" button. Cache results per (job, turn, variation).
Queue these behind the job semaphore or give them a separate concurrent slot; they are single
positions, so a separate slot is fine.

**Verify.** Explore two moves deep; evaluations match a CLI query of the same position.

### B4. "Test engine" button (S)

**Build.** `/api/engine/benchmark` runs `katago benchmark -model … -config … -time 20` (or
`-visits`) in the background and streams the summary line ("… visits/s"). Show it in the Engine
settings card with the backend name. Refuse while jobs run.

**Verify.** Metal setup reports several times the visits/s of KaTrain's OpenCL build.

### B5. Persist jobs across restarts (M)

**Why.** Jobs live in memory; the files panel only partially compensates.

**Build.** Write `jobs.json` (id, file name, created, state, report/json paths, summary) in the
Application Support folder on every state change; on launch, recreate Done jobs from it and
reload their JSON lazily on first view (reuse `load_file`). Cap at the last 50.

**Verify.** Quit and relaunch; the Analyses table still lists earlier games and the live view
opens them.

### B6. Drag-and-drop and SGF file association (M)

**Build.** Drag-and-drop onto the page: `dragover`/`drop` handlers feeding the same upload
code. File association: `CFBundleDocumentTypes` for `.sgf` in `make_app.sh`'s Info.plist; on
macOS the opened file arrives as an Apple Event, which tao/wry do not surface directly — simplest
path: a tiny AppleScript-free approach is not available, so handle it via the `open` event in
`window.rs` if tao exposes `Event::Opened { urls }` (tao 0.30+ has `Event::Opened` on macOS);
otherwise fall back to a Services menu item. Verify tao's current API first.

**Verify.** Dropping two SGFs queues two jobs; double-clicking an SGF in Finder opens Go Teacher
and queues it.

### B7. Watch folder (M)

**Build.** Settings field "Watch folder"; a task polls it every 10 s (or uses `notify` crate) for
new `.sgf` files not yet in a `seen.json`, and queues them with the default visits and profile.
Suggested default: KaTrain's `sgf_save` directory read from `~/.katrain/config.json`.

**Verify.** Save a game in KaTrain; it appears queued within 10 s.

### B8. Direct export to the lesson agent (M)

**Build.** Button "Prepare lesson bundle": writes `<report>.bundle.zip` containing the report,
`parsed.json` (run the skill's parser from Rust by shelling out to python3, or port the parser),
and the skill's `SKILL.md`, then reveals it in Finder. Optional: settings field for an HTTP
endpoint to POST the bundle to (local agent). Keep the skill as the source of truth; do not
duplicate its logic in Rust.

**Verify.** The zip opens; the parser output matches running the script manually.

### B9. Mid-game setup stones and rectangular boards (S)

**Build.** `sgf.rs`: setup stones after the first move are currently dropped with a warning;
KataGo cannot represent them either, so the honest fix is to stop analysis at that node and say
so in the report. Rectangular boards: `index.html` star points assume square; use per-axis
star lists like `board.rs` does.

**Verify.** A 19x13 SGF renders; a file with mid-game AB shows the truncation note.

---

## C. Reliability and engineering

### C1. Engine crash recovery (S)

**Build.** When `Engine::is_alive()` turns false outside a shutdown, set `EngineState::Failed`
with the reason, then call `start_engine` once automatically; jobs that were running fail with
"engine restarted — re-queue" and are re-queued once if they had not been cancelled.

**Verify.** `kill -9` the KataGo process mid-job; the engine restarts and the job completes on
retry.

### C2. Resume partial analyses (M)

**Build.** On cancel or engine death, keep the `Vec<Option<TurnEval>>`; allow "Resume" on a
cancelled job which sends a new query with `analyzeTurns` = the missing turns only. Report
rendering must tolerate gaps (render "not analysed" lines) if the user asks for a partial report.

**Verify.** Cancel at 50%, resume, final report identical to an uninterrupted run.

### C3. Fake-engine integration tests (M)

**Build.** Record one real KataGo response stream (JSON lines) for the 9x9 sample; a test
binary/fixture `tests/fake_katago.rs` that reads queries and replays the recorded responses
by turn; tests for `analyze_game`, `teaching::select`, `render_markdown` (golden file), and the
HTTP API (upload → done → report) using the fake. Run in CI without a GPU.

**Verify.** `cargo test` passes on a machine with no KataGo.

### C4. Report schema versioning (S)

**Build.** Line 3 of the report: `Report format: 3` (bump on any structural change).
`parse_review.py` reads it and refuses versions it does not know, with a clear message. Keep a
CHANGELOG of format versions in `references/katago-review-format.md`.

**Verify.** Parser rejects a report with a future version number.

### C5. Signing and notarization (S, needs an Apple Developer account)

**Build.** `make_app.sh` gains optional `CODESIGN_IDENTITY` and `NOTARY_PROFILE` environment
variables; when set, sign with hardened runtime and run `xcrun notarytool submit … --wait` and
`stapler`. Document the one-time `notarytool store-credentials` step.

**Verify.** DMG opens on another Mac without the right-click → Open dance.

### C6. Memory: spill completed jobs (M)

**Build.** After a job finishes and its JSON is written, drop `live` and `markdown` from memory
and reload from the JSON/MD files on demand (the `load_file` path already exists). Keep only the
summary in memory. Same for loaded jobs after N minutes idle.

**Verify.** Analyse ten 19x19 games; RSS stays flat.

### C7. Throughput tuning (S)

**Build.** Benchmark `numAnalysisThreads` 2/4/6 with `nnMaxBatchSize` 8/4 on the Metal mux
config; pick the best for the embedded `resources/analysis.cfg`. The one-at-a-time job queue is
right for ordering and does not imply one position at a time.

**Verify.** Positions per second for a 100-move game, engine alone on the GPU.

### C8. Small fixes noticed along the way

- Atari list: restrict to groups touching the played stone or the opponent's last move (S).
- `ForeignEngine` detection: also detect Sabaki/Lizzie-launched engines by `-config` path (S).
- `pv_len` 10 in data, 6 in tables — make both configurable in settings (S).
- Human-profile dropdown: remember the last used value per student name (S).
- Log rotation for `~/Library/Logs/GoTeacher.log` (S).

---

## Skill (go-game-teacher) follow-ups

- Consume A2 refutations: lesson `variation_explanation` walks the refutation, and puzzles'
  wrong-move explanations quote the punishment (S once A2 exists).
- Consume A3 themes and A4 status changes as first-class fields (S).
- Golden test: `tests/` with one report and the expected `parsed.json`; run in CI (S).
- Lesson JSON validator (`scripts/validate_lesson.py`): coordinates on the board, no stone
  overlaps, correct moves on empty points, wrong moves distinct from correct ones, every
  referenced move number exists in the game (M). GLM-class models benefit most from this.

---

## Priorities

1. **A2 refutation lines** — changes what the teacher can say; medium effort.
2. **A1 two-pass analysis** — better numbers on the moves that matter at the same cost.
3. **B1 chart fix** — the one display that is actively misleading for beginner games; small.
4. **A3 + A4** — cheap, and they make the skill's job mechanical.
5. **C1 + C3** — reliability and a test harness before the codebase grows further.
6. **B5, B6, B7** — quality of life; do after the above.
7. **A5, A6, B3** — larger features; pick by interest.
