# Go Teacher v5 — Specification: the eleven new computations (brief items 1–11), revised after review

This revision keeps the structure of the first draft and fixes every blocker/major from the two reviews: one perspective rule for every printed number, one parser-stable line grammar (sequences included), a guaranteed panel × level matrix for the lesson UI, an explicit token budget, a sente rule that cannot call a punished mistake "sente", relative urgency labels, life-and-death verdicts read from the best-candidate ownership over merged units, rank-typical (sampled, seeded) replays, an implementable budget planner, corrected rank arithmetic, and explicit "not computed" markers. Former open questions are decided in §16.

---

## 0. Ground rules and shared infrastructure

### 0.1 Facts the design relies on (from code and brief)
- Every KataGo number we store — `winrate`, `scoreLead`, `ownership`, per-candidate `ownership`, `utility` (sign to verify, §15.2) — is Black-view because we launch with `reportAnalysisWinratesAs=BLACK`. `sign(M) = +1` for Black, `−1` for White. "Mover-view points" = `sign(M) × (Black-view difference)`.
- `turns[k]` = evaluation of the position *before* move `k+1` (turn 0 = initial position). For `MoveReview.number = N`: before = `turns[N-1]`, after = `turns[N]`. `TurnEval.to_move` comes from `rootInfo.currentPlayer`.
- Existing passes: pass 1 (150 visits in two-pass mode, else `max_visits`), deep pass 2 over `deep_turns` (1000 visits), target-profile 1-visit pass. `analysis.rs` already has `QueryExtras { allow_moves, allow_depth, include_moves_ownership, human_profile, priority, extra_moves }`, `build_query_ext`, `run_query_ext`, `Candidate { utility, lcb, ownership }`. Build on these.
- Cost model: `t(v) ≈ a + v / r` seconds per probe position, initial `a = 0.1 s`, `r = 100 visits/s` (5 s per 500 visits with the GPU shared). These two constants are **re-fitted at run time** from `ProbeStats` (§12.2) — §15.1 (throughput) is the first verification task because it sets `budget_seconds` and the 2-in-flight decision. KataGo's `numAnalysisThreads` is not changed; probes use at most 2 in flight.
- Human model: `engine.config.human_model.is_some()` gates everything human-related (§5 human part, §8, §9, §11, the "level" replays). Without it every human field is `None`/empty and every human line becomes a `not available (no human model)` marker (§0.7), never silence.
- SGF ranks: `game.rank_black` / `game.rank_white` (BR/WR). Engine detection: `teaching::looks_like_engine`.

### 0.2 Module layout
- New `src/probes.rs`: every engine call beyond the three existing passes — `run_probe`, `eval_position`, `pass_value`, `local_life_death`, `human_policy_at`, `replay`, `rank_ladder`, `key_positions_ownership`. Each returns `Result<T>`; the orchestrator turns `Err` into an `engine_warnings` entry plus a `None`/empty result and a `not available (engine error)` marker. A probe never fails the job.
- `src/teaching.rs`: pure derivations — `sente_of`, `difficulty_of`, `plan_diff`, `missed_opportunities`, `rank_posterior`, `resolve_profiles`, `rank_value`/`profile_of`, `build_tree`, `units_of`, `choose_ld_groups`, `ld_region`, `urgency_labels`, `panel_matrix`.
- `src/analysis.rs::analyze_game`: orchestrates the stages of §12 and merges results into `TurnEval` / `MoveReview` / `TeachingCandidate` / `GameAnalysis`.
- `src/report.rs`: `REPORT_FORMAT = 5`; new line grammar of §0.5–0.7; token budget of §0.8.
- Skill: `parse_review.py` gains the fields listed in §0.9; `validate_lesson.py` gains the `from_your_game` puzzle rule (§6.7).

### 0.3 Probe execution helpers (probes.rs)
```rust
pub struct ProbeCtx<'a> {
    pub engine: &'a Engine, pub game: &'a GameRecord, pub rules: &'a str, pub komi: f64,
    pub opts: &'a AnalysisOptions, pub cancel: &'a CancellationToken,
    pub plan: std::sync::Mutex<BudgetPlan>,                 // §12.2
    pub sem: std::sync::Arc<tokio::sync::Semaphore>,        // permits = probes.in_flight (default 2)
    pub warnings: std::sync::Mutex<Vec<String>>,
    pub stats: std::sync::Mutex<ProbeStats>,
}
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct ProbeStats {
    pub queries: usize,
    pub seconds_by_kind: std::collections::BTreeMap<String, f64>,
    pub fitted_overhead_s: f64, pub fitted_visits_per_s: f64,   // a, r after calibration
    pub skipped_for_budget: Vec<String>,                          // batch ids, human-readable
    pub errors: Vec<String>,
}

/// Owns the query id for its whole life: registers, awaits, and ALWAYS unregisters (Ok, Err,
/// cancel, timeout). Fixes the dangling-sender problem of wrapping `run_query_ext` in a timeout.
pub async fn run_probe(ctx: &ProbeCtx<'_>, kind: &str, query: serde_json::Value, wanted: &[usize],
                       timeout: std::time::Duration) -> Result<Vec<TurnEval>> {
    let _permit = ctx.sem.acquire().await?;
    let (id, mut rx) = ctx.engine.query(query).await?;
    let t0 = Instant::now();
    let res = tokio::time::timeout(timeout, collect_turns(ctx, &mut rx, wanted)).await;
    let out = match res {
        Ok(r) => r,
        Err(_) => { let _ = ctx.engine.terminate(&id).await; Err(anyhow!("probe {} timed out after {:?}", kind, timeout)) }
    };
    ctx.engine.unregister(&id);
    ctx.stats.lock().unwrap().record(kind, t0.elapsed().as_secs_f64(), out.is_err());
    out
}

/// One position: the game truncated to `prefix_len` moves plus `extra` moves; analyses exactly that turn.
pub async fn eval_position(ctx: &ProbeCtx<'_>, kind: &str, prefix_len: usize,
    extra: &[(Color, Option<Coord>)], visits: u64, mut extras: QueryExtras) -> Result<TurnEval>
```
`eval_position`: clone the game, truncate `moves` to `prefix_len`, `extras.extra_moves = extra.to_vec()`, `extras.priority = 5` (main passes 0; change `server.rs::explore` to priority 10 so the live UI stays snappy), `analyzeTurns = [prefix_len + extra.len()]`, timeout 90 s. `collect_turns` handles `error`/`warning`/`isDuringSearch` exactly like `run_query_ext` and awaits `ctx.cancel`.

Colour of appended moves: alternate from `turns[prefix_len].to_move`; explicitly inserted passes (§1, §6) are `(colour, None)`. Legality: only ever append (a) `pass`, (b) a move from the parent's `moveInfos`, (c) a policy/humanPolicy index with value ≥ 0. KataGo enforces ko/superko per `rules`. Any `error` → `Err` → warning → marker.

Turn-number semantics: results are keyed by the *game* turn they belong to, never by the analysed turn number.

### 0.4 New options
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeOptions {
    #[serde(default = "t")]    pub enabled: bool,             // true
    #[serde(default = "d300")] pub budget_seconds: u64,       // 300; 0 = unlimited
    #[serde(default = "d4")]   pub featured: usize,           // 4
    #[serde(default = "d2")]   pub in_flight: usize,          // 1 or 2
    #[serde(default = "t")]    pub tree: bool,
    #[serde(default = "t")]    pub replays: bool,
    #[serde(default = "t")]    pub local_ld: bool,
    #[serde(default = "t")]    pub rank_estimate: bool,
    #[serde(default = "t")]    pub temperature_curve: bool,
    #[serde(default = "d12")]  pub panel_plies: usize,        // uniform length of every panel replay (fixed 12)
    #[serde(default = "d24")]  pub long_run_plies: usize,     // 20–30, featured only
    #[serde(default = "typical")] pub replay_mode: String,    // "typical" | "realistic"
    #[serde(default)]          pub refutation_thinking: bool, // evaluate the utility-weighted "thinking" reply too
    #[serde(default)]          pub opponent_profile: Option<String>,
}
// AnalysisOptions gains: #[serde(default)] pub probes: ProbeOptions,
```
`featured` = the first `featured` teaching candidates ordered by (`!decided_before`, `point_loss` desc). Every teaching candidate gets the **plain set** (pass probe, tree children, panel matrix, difficulty, plan diff, sente); featured ones additionally get grandchildren, local L&D, human refutation evaluation and long runs. `TeachingCandidate.featured: bool` is stored and printed.

### 0.5 Perspective rule (applies to every new Markdown line; enforced by a renderer test)
1. A **score** is always Black-view and written `B+x.x` / `W+x.x` (`lead()`).
2. A **gain, loss, value, urgency or cost** is always a positive number followed by `pts for <Colour>` with the beneficiary named: `gains 6.1 pts for Black vs passing`, `loses 7.6 pts for Black`, `ignoring it costs 2.9 pts for White`. Never a bare signed number, never a parenthesised delta.
3. Winrates are Black's, written `nn%`.
4. `eval_best` (§1) is the **root** score `turns[i].score_lead`, so prose and formula agree.
Renderer unit test: within `## Teaching candidates`, `## Missed opportunities` and `## Rank estimate`, assert no match for `\((?:−|-|\+)?[0-9]+\.[0-9]\)` and no match for `(?<![BW])[+−-][0-9]+\.[0-9] pts`.

### 0.6 Canonical sequence line (one grammar for every playable line)
```
- Sequence <kind> (<Colour> first, from the position <before|after> move <N>): <M1> <M2> … <Mk> → <B+x.x|W+x.x> [<k> moves; <note>]
```
- `<kind>` ∈ `better_line | engine_refutation | human_refutation | thinking_refutation | killing | saving | defender_answer | replay_played_student | replay_best_student | replay_played_target | replay_best_target | replay_played_opponent | replay_best_opponent | long_run_played | long_run_best | tree_child_1..4`.
- The move list is **complete** (no ellipsis; ≤ 30 tokens). `pass` may appear **only as the last token**. PVs are truncated at their first pass (note `cut at a pass`); a sampled pass ends a replay (note `stopped: pass`).
- `<note>` is `;`-separated fixed fragments: `visits <n>`, `stopped: <reason>`, `cut at a pass`, `mode typical|realistic`, `profile <name>`, `most common at <profile> (<p>%)`.
- Regex (parse_review.py): `^- Sequence (\w+) \((Black|White) first, from the position (before|after) move (\d+)\): ((?:(?:[A-T]\d{1,2}|pass) ?)+)→ ([BW]\+\d+\.\d) \[(\d+) moves(?:; ([^\]]*))?\]$`
- Every struct that carries a sequence stores `first_to_move: Color` and `start: SeqStart { turn: usize, after_played: bool }`.
- The existing `Better line (Black first): …` and `What the move allows (White first): … → B+x` lines are kept for format-4 compatibility **and** re-emitted as `Sequence better_line` / `Sequence engine_refutation` so one regex covers all playable material.

### 0.7 Absence markers (the skill must never confuse "not computed" with "none")
```
- Sequence <kind>: not available (<reason>)
- <Label>: not available (<reason>)
```
`<reason>` ∈ `no human model | probe budget | engine error | opponent is an engine of unknown strength | not featured | position decided | same as your level | too few moves`. Regex: `^- (Sequence \w+|[A-Z][\w /-]+): not available \(([^)]+)\)$`. One line at the top of `## Teaching candidates`:
```
Evidence available: urgency, move values, sente/gote, plan difference, difficulty, variation tree (children), panel sequences (engine, your level, target level, this opponent). Featured candidates (moves 23, 31, 37, 41) also have: replies (tree depth 2), life and death, human refutation, long runs. Not computed: life and death for moves 45, 52 (probe budget); long runs (probe budget).
```
Reading-guide bullet: "Missing evidence means *unknown*, never *none*."

### 0.8 Token budget (brief: "hand the agent less, not more")
Targets for a 250-move, 8-candidate report: whole report ≤ 23,000 words (≈ 30k tokens); per featured candidate block ≤ 44 lines; per plain block ≤ 26 lines; `## Missed opportunities` ≤ 8 rows; `## Rank estimate` ≤ 12 lines. Paid for by:
1. Teaching candidates and praised moves get **no** full `### Move N` entry in "Move-by-move analysis" (one line `- **Move N** … see Candidate block`); the candidate block absorbs the "position before the move" diagram and the candidate table (≤ 5 rows, PV ≤ 6 moves).
2. Opponent full entries 6 → 3; checkpoints unchanged.
3. Tree: children only in Markdown (depth 1, one line each); grandchildren JSON-only.
4. Replays: no per-ply trajectory in Markdown; `HumanRefutation.alternatives`, `Difficulty.policy_entropy/eff`, `plausibility`, `local_stake` JSON-only.
5. Temperature curve: a column in the existing Game-flow table, not a section.
Renderer test: a synthetic 250-move analysis with 8 candidates (4 featured, all probes filled) renders ≤ 23,000 words; each candidate block within its line cap.

### 0.9 Parser fields added to `parse_review.py` (per teaching candidate unless noted)
`featured`, `urgency {pts, for, label, baseline, pass_reply, pass_score}`, `move_values[] {move, gain, for, sente, reply, played, best}`, `sente_played`, `sente_best {class, reply, tenuki_cost, shared_biggest_point}`, `plan_diff {best, played, regions[] {region, delta, for}, other, net, groups[]}`, `difficulty {label, n_good, second_best, second_loss, low_confidence, human {profile, top, prob, top_loss, expected_student, expected_target}}`, `local_ld[] {color, anchor, stones, status, killing, saving, defender_answer, global_before, global_after, reconciliation_code, region[]}`, `tree[] {rank, move, label, score, loss, sente, reply}`, `human_refutation {profile, reply, prob, score, engine_score, unpunished}`, `sequences{kind → {first, start, moves[], score, note}}`, `not_available[] {what, reason}`; game-level `missed[]`, `rank_estimate`, `evidence_available`, `game_flow[].move_value`.

### 0.10 Profile resolution (`teaching::resolve_profiles`) and rank arithmetic
```rust
/// 1k and 1d are adjacent: kyu k → −k, dan d → d−1. Clamped to [−20, 8] (20k … 9d).
pub fn rank_value(profile_or_rank: &str) -> Option<i32>;      // "rank_10k"|"10k"|"10 kyu"|"10K" → −10; "2d"|"rank_2d"|"2 dan" → 1; "Np" → 8
pub fn profile_of(v: i32) -> String;                            // −10 → "rank_10k", 1 → "rank_2d", 0 → "rank_1d", −1 → "rank_1k"
pub fn stronger(v: i32, n: i32) -> i32 { (v + n).clamp(-20, 8) }
```
`rank_Nk` exists for every N in 1..20 and `rank_Nd` for 1..9, so no snapping. Unit tests: `10k → 7k`, `2k → 2d`, `1k → 3d`, `8d → 9d` (`stronger(·, 3)`); `rank_value("5-kyu") = −5`; `rank_value("rank_3d") = 2`; `profile_of(rank_value(x)) = x` for all 29 profiles.
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profiles {
    pub student: Option<String>, pub target: Option<String>, pub opponent: Option<String>,
    pub pair: Option<String>,             // "rank_{B}_{W}", Black first, when both colours have a rank profile
    pub student_source: String, pub opponent_source: String,
    // "chosen at upload" | "SGF rank" | "rank in the bot's name" | "estimated from the moves" | "same as student (bot strength unknown)" | "none"
    pub opponent_is_engine: bool,
}
```
Rules, in order: `student = opts.human_profile` else `rank_estimate.student.estimate` (after §11) else `None`. `target = opts.human_profile_target` else `profile_of(stronger(rank_value(student), 3))`. `opponent = opts.probes.opponent_profile` else `profile_of(rank_value(sgf rank of the opponent))` else (engine name with a parsable rank, e.g. `AI (Calibrated Rank 8k)`) that rank else (engine name without a rank) `student` with source "same as student (bot strength unknown)" else `rank_estimate.opponent.estimate` else `student`. Never `Side::Engine`: a 48-visit KataGo is several dan stronger than any weakened bot the student actually faced. `pair = Some(format!("rank_{}_{}", strip(profile of Black), strip(profile of White)))` when both exist (shape verified: `rank_10k_3k`).

---

## 1. Urgency via pass value

**Definition.** For `turns[i]` with mover `M`: `eval_best = sign(M) × turns[i].score_lead` (root), `eval_pass = sign(M) × score_lead(P + [M pass])`, `urgency_raw = eval_best − eval_pass`, `urgency = max(0, urgency_raw)`. This is the value of having the move: how many points `M` loses by handing the turn over, with the opponent then playing their best move `pass_reply = candidates[0].mv` of the pass position ("had Black passed, White would play C3").

**Query.** `eval_position(ctx, "pass", i, [(M, None)], 400, default)`. 400 visits (≈ 4.1 s): the pass position is easy (a free move) and stabilises fast; the reply needs to be right, not read out.

**Positions probed** (`pass_turns`, deduplicated, in this priority order, capped at 16 full probes; anything past the cap goes to `skipped_for_budget`): (a) `t.number − 1` for teaching candidates, featured first; (b) the positions after the opponent's 4 biggest errors (`r.number` of those reviews; they are the student's before-positions and already in `deep_turns`) — this lets §10 find BigMove moments that are not teaching candidates; (c) endgame extras of §2; (d) `p.number − 1` for praised moves. Plus the temperature curve: every turn `k ≡ 0 (mod 10)` with `0.01 ≤ turns[k].winrate ≤ 0.99` at 100 visits (≈ 1.1 s each) when `probes.temperature_curve`.

**Baseline and labels (`teaching::urgency_labels`).** Absolute thresholds are useless before the endgame: on an empty 19x19 board urgency ≈ 14 pts (≈ 2 × komi) and stays above 8 through most of the middlegame. So the label is relative: `baseline(i)` = median of curve samples (and full pass probes) within ±15 moves of `i` (≥ 2 samples), else within ±30, else the median of the whole curve; `None` if no curve. `urgent` if `urgency ≥ 1.3 × baseline`, `quiet` if `urgency ≤ 0.6 × baseline`, else `normal`; with no baseline: label `unknown`. `urgency_raw < −1.0` → `urgency = 0`, note `nothing better than passing (dame or seki)`.

**Storage.**
```rust
// TurnEval
#[serde(default, skip_serializing_if = "Option::is_none")] pub pass_probe: Option<PassProbe>,
pub struct PassProbe { pub score_lead: f64 /* Black view after the pass */, pub winrate: f64, pub visits: u64,
    pub reply: Option<String>, pub reply_pv: Vec<String> /* ≤ 6, cut at pass */ }
// MoveReview (position before the move, mover view)
#[serde(default)] pub urgency: Option<f64>, #[serde(default)] pub urgency_raw: Option<f64>,
#[serde(default)] pub urgency_label: Option<String>, #[serde(default)] pub urgency_baseline: Option<f64>,
#[serde(default)] pub played_value: Option<f64>,   // §2
// GameAnalysis
#[serde(default)] pub temperature: Vec<(usize, f64)>,  // (turn, urgency for the side to move)
```
**Markdown (one line per candidate; regex-stable).**
`- Urgency: 9.4 pts for Black (urgent; typical at this stage 6.0); if Black passes, White plays C3 → W+7.3.`
Regex: `^- Urgency: (\d+\.\d) pts for (Black|White) \((urgent|normal|quiet|unknown)(?:; typical at this stage (\d+\.\d))?\); if (Black|White) passes, (Black|White) plays ([A-T]\d+|pass) → ([BW]\+\d+\.\d)\.$`. Zero case: `- Urgency: 0.0 pts for Black (quiet; nothing better than passing: dame or seki).`
Game-flow table gains a column `Move value (pts, for side to move)`, filled where a curve sample or pass probe exists, `—` elsewhere. Praise table gains `Urgency (for <colour>)` and `Difficulty` columns.

**Edge cases.** If `game.moves[i−1]` is a pass, do **not** probe (two consecutive passes: terminal under area rules, encore under Japanese rules — either way not a pass value); `pass_probe = None`, marker `Urgency: not available (previous move was a pass)`. Keep this rule regardless of §15.3's outcome. Decided positions: points stay meaningful; always compute from `scoreLead`.

**Cost.** ≤ 16 × 4.1 ≈ 66 s + curve (9x9: 7 samples ≈ 8 s; 19x19: 25 samples ≈ 28 s).

---

## 2. Endgame move values (gain of each candidate vs passing)

**Definition.** `value(c) = sign(M) × (c.score_lead − pass.score_lead)` for each `c ∈ turns[i].candidates` with `c.visits ≥ max(20, 0.03 × root visits)`. `value(root) = urgency`. `played_value = sign(M) × (turns[i+1].score_lead − pass.score_lead)` from the actual after-position (deep for teaching candidates). Identity by construction: `urgency − played_value = point_loss` exactly; the candidate-based `value(best)` differs from `urgency` by the root-vs-child gap (typically ≤ 1 pt) — report `value(best)` from the candidate and `urgency` from the root, and test `|value(best) − urgency| ≤ 1.5` on 20 positions (§15.7).

**Meaning (reading guide).** "How much the move gains compared with passing, both sides then playing well. It sits between the miai value and the deiri (full swing) value: close to miai while many moves of similar size remain, close to deiri late in the endgame when one big move is left. A sente move shows a high gain even when its local swing is small, because it keeps the turn."

**Positions.** All teaching candidates; additionally when `phase == "endgame"`: student moves with `point_loss ≥ 1.0` that are not candidates, up to 4, largest loss first (pass_turns class (c)).

**Storage.**
```rust
pub struct MoveValue { pub mv: String, pub gain: f64 /* mover view, may be negative in JSON */, pub visits: u64,
    pub played: bool, pub best: bool, pub sente: Option<SenteClass>, pub reply: Option<String>, pub source: String /* "candidate" | "after-position" */ }
// MoveReview: #[serde(default)] pub move_values: Vec<MoveValue>  (sorted by gain desc, ≤ 6; played always included when known)
```
**Markdown** (≤ 4 lines: best, played, and the next two by gain):
`- Move value D4: gains 9.4 pts for Black vs passing; sente; White replies C4; best.`
`- Move value G7: gains 1.8 pts for Black vs passing; gote; White replies D4; played.`
Negative gains print as `loses 0.6 pts for Black vs passing`. Regex: `^- Move value ([A-T]\d+|pass): (gains|loses) (\d+\.\d) pts for (Black|White) vs passing; (sente|gote|punished locally|unclear)(?:; (Black|White) replies ([A-T]\d+|pass))?(?:; (played|best|played, best))?\.$`

**Cost.** 0 s beyond §1 (≤ 4 extra pass probes for the endgame table).

---

## 3. Sente / gote classification

**Go definition.** A move by `M` is *sente* when it carries a threat the opponent `O` must answer locally, and the exchange (move + answer) costs `M` nothing, so `M` keeps the initiative. A move whose best local reply is an *attack* (cut, atari, kill) is not sente — it is a mistake being punished. A move `O` can ignore (best reply elsewhere) is *gote*.

**Inputs for a move `c` by `M` at root `turns[i]` (root score `s0`, Black view)**, reply position `Q` (opponent `O` to move, candidates `q_k`):
- `loss_vs_best(c)`: for the played move `point_loss`; for a candidate `sign(M) × (candidates[0].score_lead − c.score_lead)`.
- `best_reply = q_0.mv`; `reply_local = is_local(q_0.mv, c, board after c)`.
- `is_local(reply, c, board)`: `reply != pass` and (Chebyshev `≤ R` with `R = 2` for boards ≤ 9, `3` for ≤ 13, `4` for 19x19 — a hane four lines away is still local on 19x19; **or** `reply` is a liberty of the chain containing `c`; **or** `reply` is a liberty of an enemy chain adjacent to `c`).
- `local_share = Σ visits(q_k local) / Σ visits(q_k)`.
- `tenuki_cost = sign(O) × (q_0.score_lead − q_far.score_lead)` with `q_far` = best non-local candidate with `visits ≥ 10`; `None` if none (then `tenuki_bound = sign(O) × (q_0.score_lead − q_last.score_lead)`, `bound: true`).
- `exchange_gain = sign(M) × (q_0.score_lead − s0)`: what `M` gains from the exchange `c, q_0` (≈ 0 for the best move by minimax; ≈ `−point_loss` for a punished mistake). Stored as evidence.
- `shared_biggest_point` (rename of `mutual_point`): `is_local(turns[i].pass_probe.reply, c)` — the opponent's biggest move was at the same spot ("the point both players wanted most").

**Classification.**
```rust
pub enum SenteClass { Sente, ProbablySente, Gote, PunishedLocally, Unclear }
```
1. `Unclear` if `Q` has no candidates or `q_0.visits < 20`.
2. If `loss_vs_best(c) ≥ 1.5` (an inaccuracy or worse) **or** `exchange_gain < −1.5`: `PunishedLocally` if `reply_local`, else `Gote`. The word *sente* is never used for a mistake.
3. Else if `!reply_local` (including `pass`): `Gote`.
4. Else `Sente` if `tenuki_cost ≥ 1.5` (or `tenuki_bound ≥ 1.5`); `ProbablySente` otherwise (the local answer is best but not forced). 1.5 sits above the ±1 pt noise of 300-visit child searches and equals the inaccuracy threshold: ignoring a sente move is at least an inaccuracy.

**Where `Q` comes from.** Played move: `Q = turns[i+1]` (deep). Best move and other tree children: the §7 child node (300 visits). Candidates without a node: PV-only (`reply = c.pv[1]`, `tenuki_cost = None`, class from `reply_local` only, `source = "pv"`, printed with `(from the line only)`).

**Storage.**
```rust
pub struct SenteInfo { pub class: SenteClass, pub reply: Option<String>, pub reply_local: bool, pub local_share: f32,
    pub tenuki_cost: Option<f64>, pub tenuki_bound: bool, pub exchange_gain: Option<f64>, pub loss_vs_best: f64,
    pub shared_biggest_point: bool, pub source: String /* "search" | "pv" */ }
// MoveReview: played_sente: Option<SenteInfo>, best_sente: Option<SenteInfo>; TeachingCandidate copies both.
```
**Markdown (one fact per line).**
`- Sente/gote, played G7: punished locally; White's best reply D4 is a local punishment, not a forced answer; 0% of the search effort went to non-local replies.`
`- Sente/gote, best D4: sente; White must answer C4; ignoring it costs 2.9 pts for White.`
`- Sente/gote, best E3: gote; White's best reply G7 is elsewhere.`
`- Shared biggest point: yes; White's own biggest move was C3, in the same area.`
Markdown prints `sente` for both `Sente` and `ProbablySente` (the cost is always quoted, so the reader sees how forced it is); JSON keeps the distinction. Regex: `^- Sente/gote, (played|best) ([A-T]\d+|pass): (sente|gote|punished locally|unclear); (.*)$` with fixed fragment set `{"<C> must answer <mv>", "ignoring it costs <x> pts for <C>", "<C>'s best reply <mv> is elsewhere", "<C>'s best reply <mv> is a local punishment, not a forced answer", "<n>% of the search effort went to non-local replies", "from the line only"}`.

**Edge cases.** Reply `pass` → Gote. Played move is a pass → skip. 9x9: `R = 2` and the liberty clauses keep the label meaningful; reading guide notes that on 9x9 "elsewhere" means the other half of the board.

**Cost.** 0 s beyond §7.

---

## 4. Per-candidate ownership: plan differences played vs best

**Query.** `include_moves_ownership = true` on the deep pass 2 query (CPU-side averaging only). Coverage: `select` is re-run after pass 2 and after the target pass, so candidates can enter the list without ownership data. **After the final `select`**, compute `missing` = teaching before-positions whose `candidates[0].ownership.is_none()` and run one "key positions" query over them at `deep_visits` (two-pass) or `max_visits` (single-pass) with the flag (`probes::key_positions_ownership`; in single-pass mode `missing` = all teaching before-positions, ≤ 8 × 5 s). After merging, keep `Candidate.ownership` only on teaching before-positions, only for the top 3 candidates plus the played move, rounded to 2 decimals; drop everywhere else.

**Algorithm (`teaching::plan_diff`)** at teaching index `i`, mover `M`, best `b`, played `p`:
- `own_b = b.ownership`. `own_p = p.ownership` if `p ∈ candidates` with `visits ≥ 100` (a candidate ownership averaged over fewer leaves is too noisy at the 1.5-pt region threshold); else `turns[i+1].ownership` (root of the actual after-position); `played_source` records which.
- `delta[k] = sign(M) × (own_b[k] − own_p[k])`; `total = Σ delta` (**no /2**: Σ ownership ≈ area-score difference, one flipped point moves both by 2). `inconsistent = |total − point_loss| > max(3.0, 0.5 × point_loss)`. Under Japanese rules the ownership sum approximates the area difference; ±1–2 pts of slack is normal (reading guide).
- Regions: sum per `region_name`; print those with `|sum| ≥ 1.5`, then `other regions` = `total − Σ printed` so the parts add up to `Net`.
- Groups: for every unit (§6.1) of ≥ 2 stones on `boards[i]`: `mean_b`, `mean_p`, statuses via `status_of` (±0.4); report when status differs or `|mean_b − mean_p| ≥ 0.5`.
- `alt_regions` for the second-best candidate when its loss ≤ 1.0 (two good plans), JSON-only.

**Storage.**
```rust
pub struct RegionDiff { pub region: String, pub delta: f64 /* + = best move gains here for the mover */ }
pub struct GroupDiff { pub color: Color, pub anchor: String, pub stones: usize, pub own_best: f32, pub own_played: f32, pub status_best: String, pub status_played: String }
pub struct PlanDiff { pub best: String, pub played: String, pub played_source: String, pub regions: Vec<RegionDiff>, pub other: f64,
    pub groups: Vec<GroupDiff>, pub total: f64, pub inconsistent: bool, pub alt_regions: Vec<RegionDiff> }
// TeachingCandidate: plan_diff: Option<PlanDiff>
```
**Markdown.**
`- Plan difference (D4 vs played G7): lower left gains 6.2 pts for Black; centre gains 1.1 pts for Black; top side gains 2.1 pts for White; other regions gain 2.4 pts for Black; net 7.6 pts for Black.`
`- Plan difference groups: White C3 (3 stones) dead after D4, alive after G7; Black E5 (4 stones) alive after D4, unsettled after G7.`
Regex: `^- Plan difference \(([A-T]\d+) vs played ([A-T]\d+|pass)\): (.*); net (\d+\.\d) pts for (Black|White)\.$` with each region fragment `(<region>|other regions) gains? (\d+\.\d) pts for (Black|White)`. When `inconsistent`: append `; ownership sums disagree with the point loss`.

**Edge cases.** No played ownership and no `turns[i+1].ownership` → marker. `p == b` (praise) → skip.

**Cost.** 0 s in two-pass mode plus the `missing` query (≤ 3 positions typical, 15–30 s); ≤ 40 s single-pass.

---

## 5. Difficulty / sharpness

Pure derivation from `turns[i]`. Computed for every student move; reported for teaching candidates and praised moves. Praised before-positions are **added to `deep_turns`** (praise is computed from pass-1 reviews before the deep pass, 3 × 10 s) so their labels are not artefacts of a 150-visit search; additionally `low_confidence = turns[i].visits < 400` and then the label is printed with `(shallow search: N visits)`.

**Quantities** (candidates with `visits ≥ max(10, 0.02 × root visits)`): `loss_k = sign(M) × (c_0.score_lead − c_k.score_lead)`; `n_good` = count `loss_k ≤ 1.0`; `gap_12 = loss_1`; `alt_mean_loss` = mean over `k = 1..min(4, K)`; `policy_entropy`, `policy_eff = exp(H)`, `policy_top1` (JSON-only). Human (student profile): `human_top`, `human_top_prob`, `human_top_loss` (if searched), `expected_loss_student = Σ h(c_k) loss_k / Σ h(c_k)` with `coverage_student = Σ h(c_k)` (print only if ≥ 0.5); same for the target profile.

**Labels.** `OnlyMove` if `n_good == 1 && gap_12 ≥ 3.0`; `Sharp` if `n_good ≤ 2 && gap_12 ≥ 1.5`; `Wide` if `n_good ≥ 4`; else `Normal`. `trap = human_top_loss ≥ 3.0 || policy_top_loss ≥ 3.0`. Thresholds mirror the category boundaries (1.5 inaccuracy, 3 mistake).

**Storage.**
```rust
pub enum DifficultyLabel { OnlyMove, Sharp, Normal, Wide }
pub struct Difficulty { pub label: DifficultyLabel, pub visits: u64, pub low_confidence: bool, pub n_good: u8, pub gap_12: Option<f64>, pub second: Option<String>,
    pub alt_mean_loss: Option<f64>, pub policy_entropy: f32, pub policy_eff: f32, pub policy_top1: f32,
    pub human_top: Option<String>, pub human_top_prob: Option<f32>, pub human_top_loss: Option<f64>,
    pub expected_loss_student: Option<f64>, pub coverage_student: Option<f32>, pub expected_loss_target: Option<f64>, pub coverage_target: Option<f32>, pub trap: bool }
// MoveReview: difficulty: Option<Difficulty>; TeachingCandidate copies it.
```
**Markdown.**
`- Difficulty: only move; 1 move within 1 pt of the best; second best E3 loses 4.2 pts for Black.`
`- At your level (rank_10k) the most common move is G7 (38%), which loses 7.6 pts for Black: a typical trap at this level. Average loss here: 4.1 pts at rank_10k, 1.2 pts at rank_7k.`
Regex 1: `^- Difficulty: (only move|sharp|normal|wide)(?: \(shallow search: (\d+) visits\))?; (\d+) moves? within 1 pt of the best(?:; second best ([A-T]\d+|pass) loses (\d+\.\d) pts for (Black|White))?\.$`. Regex 2: `^- At your level \((\w+)\) the most common move is ([A-T]\d+|pass) \((\d+)%\)(?:, which loses (\d+\.\d) pts for (Black|White))?(: a typical trap at this level)?\.(?: Average loss here: (\d+\.\d) pts at (\w+)(?:, (\d+\.\d) pts at (\w+))?\.)?$`.
Reading-guide bullets: only move = every alternative loses ≥ 3 pts; sharp = the second choice already loses ≥ 1.5; wide = ≥ 4 moves within 1 pt.

**Cost.** 0 s (+ ≤ 3 deep positions ≈ 30 s in two-pass mode).

---

## 6. Local life-and-death via allowMoves

### 6.1 Units (`teaching::units_of`)
A beginner's "group" is several chains. Merge same-colour chains on `boards[i]` when any pair of stones is at Chebyshev 1 (diagonal contact), or forms a one-point jump (distance 2 orthogonally, middle point empty, and the two points orthogonally adjacent to the middle are empty or own colour). Units > 25 stones are skipped (a dragon is not a local problem). `units_of` is also used by §4 groups and §10.

### 6.2 Which units (`teaching::choose_ld_groups`), per featured candidate at index `i`
(a) units containing a `status_changes` group of this move; (b) units in `plan_diff.groups` whose status differs; (c) units of ≥ 3 stones within Chebyshev 2 of the played or the best move whose global status at `turns[i]` is `unsettled` (`|mean own| < 0.4`), either colour. Order (a), (b), (c); ≤ 2 per candidate, ≤ 8 per game; skip units with exactly 1 liberty (atari: trivially readable — `atari_after` already reports them). Deduplicate by anchor across candidates (analyse at the earliest).

### 6.3 Region (`teaching::ld_region`) for unit `G` (defender `D`, attacker `A`)
`R = liberties(G) ∪ {empty points within Chebyshev 2 of any stone of G} ∪ liberties of every enemy chain adjacent to G`; if `|R| > 28` shrink to Chebyshev 1. Pass only empty points plus `"pass"` (occupied points are illegal anyway); same list for both players (verified); coordinates via `Coord::to_gtp(size_y)`.

### 6.4 Two queries (`visits = 300`, `allow_depth = 12`, `include_moves_ownership = true`)
1. Attacker first: `extra = []` if `turns[i].to_move == A`, else `[(D, None)]`.
2. Defender first: `extra = []` if `to_move == D`, else `[(A, None)]`.
Guard: if `game.moves[i−1]` is a pass, run only the query whose side is already to move; the other is `None` (avoid two consecutive passes, cf. §1). `untilDepth 12` keeps the fight local for six moves each, after which the search is unrestricted so leaf values are realistic. **Never** print restricted `scoreLead`/`winrate` next to global numbers; `local_stake = |q2.score_lead − q1.score_lead|` stays JSON-only.

### 6.5 Verdict — read from the best non-pass candidate's ownership, not the root
The restricted root mixes in `pass`/wasted moves when the search judges the group "not worth the tempo"; the candidate's own ownership map is the outcome *if that move is played*. `m_att` = defender-view mean ownership over `stones(G)` of q1's best non-pass candidate with `visits ≥ 20` (`killing_move`); `m_def` likewise from q2 (`saving_move`). Fallback to root ownership only if no such candidate (`verdict_source = "root"`). `m = sign(D) × mean`.
```rust
pub enum LocalStatus { Alive, Dead, Unsettled, Unclear }
```
- `Alive` if `m_att ≥ 0.4 && m_def ≥ 0.4` (lives even when attacked first).
- `Dead` if `m_def ≤ −0.4 && m_att ≤ −0.4` (dies even moving first).
- `Unsettled` if `m_def ≥ 0.4 && m_att ≤ −0.4`: whoever moves first decides; `saving_move`/`saving_pv` from q2, `killing_move`/`killing_pv` from q1, `defender_answer_if_attacked = q1.candidates[0].pv[1]` (the "live if attacked" answer).
- `Unclear` otherwise (ko, seki, shortage of liberties outside R, contradiction `m_att ≥ 0.4 && m_def ≤ −0.4`); note `stones' ownership is ambiguous (possible ko or seki)`. No `Seki` label: seki stones are alive and owned by their colour, so ownership ≈ 0 on stones means uncertainty, not seki.
PVs are cut at their first pass (§0.6).

### 6.6 Reconciliation with the global status (`global_before = status_of(turns[i].ownership)`, `global_after = status_of(turns[i+1].ownership)`), fixed sentences with codes
| code | local | global_before | sentence |
|---|---|---|---|
| R1 | Unsettled | alive | `alive only because the opponent has no time to attack; locally it still needs a move` |
| R2 | Unsettled | dead | `the engine expects it to die because the opponent gets there first or saving it is not worth the tempo; locally it can still be saved` |
| R3 | Alive | unsettled/dead | `safe locally; the global doubt is about the surrounding fight, not its eyes` |
| R4 | Dead | any | `cannot be saved locally even moving first` |
| R5 | Unsettled | unsettled | `whoever moves first here decides the group` |
| R6 | Alive | alive | `settled` |
| R0 | Unclear | any | `local reading inconclusive (possible ko or seki)` |

### 6.7 Puzzle from the game (decision: allowed)
A new puzzle type `from_your_game: true`: position = stones before move N (already in the block), `player_to_move` = the side whose move is asked (attacker for a kill puzzle, defender for a save puzzle), `correct_moves = [killing_move]` or `[saving_move]`, wrong-move refutations from the other PV, clickable area = `region`. `validate_lesson.py` skips the "identical to the game position" check for puzzles flagged `from_your_game` whose `correct_moves[0]` equals a `killing`/`saving` move printed for that candidate. The region line is printed for that purpose.

**Storage.**
```rust
pub struct LocalLifeDeath { pub turn: usize, pub group_color: Color, pub anchor: String, pub stones: usize, pub region: Vec<String>,
    pub status: LocalStatus, pub own_attacker_first: f32, pub own_defender_first: f32, pub verdict_source: String,
    pub killing_move: Option<String>, pub killing_pv: Vec<String>, pub saving_move: Option<String>, pub saving_pv: Vec<String>,
    pub defender_answer_if_attacked: Option<String>, pub defender_answer_pv: Vec<String>,
    pub first_to_move_attack: Color, pub first_to_move_save: Color, pub inserted_pass: Option<Color>,
    pub local_stake: f64 /* JSON only */, pub global_before: String, pub global_after: String,
    pub reconciliation_code: String, pub reconciliation: String, pub visits: u64 }
// TeachingCandidate: local_ld: Vec<LocalLifeDeath>; GameAnalysis: local_ld: Vec<LocalLifeDeath> (JSON only, not printed)
```
**Markdown (per unit: one status line, ≤ 3 sequence lines, one region line).**
`- Local life and death, White C3 (3 stones), before move 23: unsettled; Black kills with D2; White saves with C2; ownership status before the move unsettled, after the played move alive; R1 alive only because the opponent has no time to attack; locally it still needs a move.`
`- Sequence killing (Black first, from the position before move 23): D2 C2 B2 D1 → W+3.1 [4 moves; local reading, restricted search]`
`- Sequence saving (White first, from the position before move 23): C2 D2 B4 → W+8.9 [3 moves; local reading, restricted search]`
`- Sequence defender_answer (Black first, from the position before move 23): D2 C2 → W+3.1 [2 moves; if Black attacks first, White answers C2]`
`- Puzzle region (White C3 group): B1 B2 C1 C2 D1 D2 D3 E2`
Regex (status): `^- Local life and death, (Black|White) ([A-T]\d+) \((\d+) stones?\), before move (\d+): (alive|dead|unsettled|unclear)(?:; (Black|White) kills with ([A-T]\d+))?(?:; (Black|White) saves with ([A-T]\d+))?; ownership status before the move (\w+), after the played move (\w+); (R\d) (.*)\.$`. The restricted scores in these sequence lines carry the note `local reading, restricted search`; the reading guide says they are not comparable with any other number in the report.

**Edge cases.** Student = White: `D`/`A` by unit colour. 9x9: R may cover most of the board (verdict ≈ global; fine). Rectangular boards via `to_gtp(size_y)`. Either query erroring → unit skipped, marker `Local life and death: not available (engine error)`.

**Cost.** ≤ 8 units × 2 × 3.1 s ≈ 50 s (typical 4–6 units → 25–37 s).

---

## 7. Small variation tree per teaching moment

**Node set** at teaching index `i`, mover `M`, root `turns[i]`:
- Children (all candidates): top 3 candidates by `order` plus the played move if not among them (≤ 4).
- Grandchildren (featured only): the top 2 replies of each child's own analysis (≤ 8); the third ply is each grandchild's `pv[0..4]`, shown in JSON only.
- Extra labelled children of the played node: `HumanReply` (§8 most-common reply), `StudentLevelReply`, `TargetLevelReply` (first plies of the §9 panel replays) when they differ from the two engine replies — JSON-only.

**Piggyback.** Played child = `turns[i+1]` (deep); its top-2 replies = `turns[i+1].candidates[0..2]`; if the game's reply equals one of them, that grandchild = `turns[i+2]`. Everything else: `eval_position(ctx, "tree", i, [c], 300, default)` for children (≈ 3.1 s), `eval_position(ctx, "tree", i, [c, r], 150, default)` for grandchildren (≈ 1.6 s; the node's value is what matters). A candidate's children are issued as one batch (2 in flight).

**Per node.** `moves` from the root, `score_lead`/`winrate` (Black view), `visits`, `loss` = **positive** mover-`M`-view points below the root's best (`max(0, sign(M) × (root.score_lead − node.score_lead))` for children; for grandchildren the loss of the line), `pv` (≤ 4), `label`, `human_prob`, `sente` for children (§3), `first_to_move`, `source`.

```rust
pub enum NodeLabel { Best, Played, Alternative, BestReply, SecondReply, HumanReply, StudentLevelReply, TargetLevelReply }
pub struct VariationNode { pub moves: Vec<String>, pub first_to_move: Color, pub to_move: Color, pub winrate: f64, pub score_lead: f64, pub visits: u64,
    pub loss: f64, pub pv: Vec<String>, pub label: NodeLabel, pub human_prob: Option<f32>, pub sente: Option<SenteInfo>, pub source: String, pub children: Vec<VariationNode> }
pub struct VariationTree { pub turn: usize, pub mover: Color, pub root_score: f64, pub root_winrate: f64, pub depth: u8, pub children: Vec<VariationNode> }
// TeachingCandidate: tree: Option<VariationTree>
```
**Markdown (children only; one line per child, plus its sequence line).**
`- Tree child 1 D4 [best]: B+2.0; sente; White replies C4.`
`- Sequence tree_child_1 (Black first, from the position before move 23): D4 C4 E3 D2 F3 → B+2.0 [5 moves; visits 300]`
`- Tree child 3 G7 [played]: W+5.5; loses 7.6 pts for Black; punished locally; White replies D4.`
Regex: `^- Tree child (\d) ([A-T]\d+|pass) \[(best|played|alternative)\]: ([BW]\+\d+\.\d)(?:; loses (\d+\.\d) pts for (Black|White))?; (sente|gote|punished locally|unclear)(?:; (Black|White) replies ([A-T]\d+|pass))?\.$`. Grandchildren (loss of the line, tenuki costs) live in JSON.

**Edge cases.** Praise: children = best + 2 alternatives, depth 1. `pass` as a child is allowed. Decided positions: built (cheap), `decided` flag. Children are legal by construction.

**Cost.** Featured: ≤ 3 probed children × 3.1 + ≤ 6 grandchildren × 1.6 ≈ 19 s; plain ≈ 9.3 s. 4 + 4 ≈ 113 s sequential; ≈ 70 s with 2 in flight and GPU headroom.

---

## 8. Realistic refutations (human network at the opponent's rank)

**Profile.** `profiles.pair` when available, else `profiles.opponent`. If `opponent_source == "same as student (bot strength unknown)"` the line is still produced but labelled `opponent modelled at your level (bot strength unknown)`.

**Step 1 — human policy after each candidate's move.** One query on the game move list, `analyzeTurns = [t.number for all teaching candidates]`, `maxVisits = 1`, `human_profile = Some(profile)`, priority 5 (≈ 0.1 s/turn). Store `TurnEval.human_policy_opponent` on those turns only.

**Step 2 — choose the reply** (position `Q = turns[i+1]`, opponent `O`), deterministic:
- `most_common` (**printed**): `argmax humanPolicy` over legal entries (pass only if > 0.5). It is a legitimate, explainable fact ("41% of 8k players play F7 here") and is what the report claims.
- `thinking` (JSON; evaluated only if `probes.refutation_thinking`): over searched `q_k` with `visits ≥ 5`, `w_k = h(q_k) × exp(u_O(q_k) / 0.5)`, `u_O = sign(O) × q_k.utility` (sign per §15.2); if the human top move (`h ≥ 0.25`) is unsearched, add it with `u_O = u_O(q_last)`. Labelled in the report, when printed, as `a thinking <profile> (human moves filtered by the engine)`.
Top-3 by probability with `plausibility = w/Σw` stored in `alternatives` (JSON).

**Step 3 — evaluate** (featured candidates): `eval_position(ctx, "human_ref", i+1, [(O, r_h)], 300, default)` → `score_lead`, `winrate`, `pv` (≤ 6, cut at pass). Piggyback: if `r_h ∈ {q_0, q_1}` the node is a §7 grandchild. The engine refutation stays `turns[i+1].candidates[0]`. This node is also the first ply of the `replay_played_opponent` panel (§9), so the panel and the refutation never disagree. Plain candidates: reply and probability only (no evaluation; marker `Sequence human_refutation: not available (not featured)`).

**Not a re-grade.** `point_loss`/`category` stay engine-based (brief exclusion). `unpunished_points = sign(M) × (score_lead(after human reply) − engine_score)` is printed as a fact, never as a grade.

**Storage.**
```rust
pub struct HumanRefutation { pub profile: String, pub profile_note: Option<String>, pub reply: String, pub prob: f32,
    pub alternatives: Vec<(String, f32, f32)>, pub thinking_reply: Option<String>, pub thinking_prob: Option<f32>,
    pub evaluated: bool, pub line: Vec<String>, pub score_lead: f64, pub winrate: f64, pub visits: u64, pub first_to_move: Color,
    pub engine_reply: String, pub engine_score: f64, pub same_as_engine: bool, pub unpunished_points: f64 }
// TeachingCandidate: human_refutation: Option<HumanRefutation>
```
**Markdown.**
`- Realistic refutation (White as rank_10k_8k): most common reply F7 (41%); engine-best reply D4.`
`- Sequence human_refutation (White first, from the position after move 23): F7 D4 E3 C4 G3 → B+0.4 [5 moves; most common at rank_10k_8k (41%); visits 300]`
`- Unpunished: the most common reply leaves 5.8 pts for Black of the punishment unplayed (B+0.4 vs W+5.4).`
Regex: `^- Realistic refutation \((Black|White) as (\w+)(?:; ([^)]+))?\): most common reply ([A-T]\d+|pass) \((\d+)%\); engine-best reply ([A-T]\d+|pass)\.$`; `^- Unpunished: the most common reply leaves (\d+\.\d) pts for (Black|White) of the punishment unplayed \(([BW]\+\d+\.\d) vs ([BW]\+\d+\.\d)\)\.$`. Same reply as the engine → `- Unpunished: the most common reply is the engine-best reply.`

**Edge cases.** `humanPolicy` length = `sx*sy + 1` (verify §15.4). Student = White: `O` = Black. No human model → markers.

**Cost.** 1 s + ≤ 4 featured evaluations × 3.1 s (fewer with piggyback) ≈ 5–14 s.

---

## 9. Counterfactual replays and the panel × level matrix

### 9.1 The matrix (guaranteed for every teaching candidate)
Panels: **after your move** (opponent first, from the position after move N) and **after the better move** (student first, from the position before move N, first token = best). Levels: `engine`, `student` (student profile both sides), `target` (target profile both sides), `opponent` (`profiles.pair`, else student vs opponent profile). Cells:

| level | after your move | after the better move |
|---|---|---|
| engine | `engine_refutation` = `turns[i+1].candidates[0]` + pv, ≤ 12 tokens (existing) | `better_line` = best + pv, ≤ 12 tokens (existing) |
| student | `replay_played_student`, 12 plies | `replay_best_student`, 12 plies |
| target | `replay_played_target` | `replay_best_target` |
| opponent | `replay_played_opponent` (first ply forced = §8 most-common reply) | `replay_best_opponent` |

All six human cells have the **same uniform length** (`panel_plies = 12`; shorter only when stopped, with `stopped: <reason>` in the note) so switching level never changes the sequence length. A cell that cannot be filled is emitted as a marker with a reason (§0.7). When `profiles.opponent == profiles.student` (bot of unknown strength) the `opponent` row is `not available (same as your level)`. Featured candidates additionally get `long_run_played` / `long_run_best` at the `opponent` level, `long_run_plies` (24) plies; their first 12 plies are identical to the panel replay (seeding is per ply, §9.2) so the long run extends rather than contradicts the panel.

### 9.2 Move choice per ply (`HumanChoice`)
- `Typical` (**default**): one 1-visit query `eval_position(ctx, "replay", prefix, line_so_far, 1, QueryExtras { human_profile: Some(p), .. })` → sample from `humanPolicy` over legal entries with `p ≥ 0.01` (renormalised; if none, the argmax), temperature 1, with a **fixed seed** so the report is reproducible: `seed = fnv1a64(format!("{}|{}|{}|{}", game_hash, turn, kind, ply))`, `game_hash = fnv1a64(move list as "B D4,W Q16,…")`, `u = splitmix64(seed) >> 11 / 2^53`, inverse-CDF over moves sorted by policy index. Argmax would collapse to the mode of the distribution — systematically stronger and more repetitive than the rank — so it is not used. Sampling at temperature 1 is the calibrated rank distribution; the 1% floor removes noise moves and slightly strengthens play (stated in the reading guide). Record `rootInfo` score/winrate (1-visit) as the trajectory point. ≈ 0.1 s/ply.
- `Realistic` (opt-in `replay_mode = "realistic"`): the 1-visit query, then top-K (`K = 6`, mass ≥ 0.02) evaluated by `eval_position(…, 48, QueryExtras { allow_moves: Some(topK), allow_depth: 1, .. })`, choose `argmax h(m) × exp(u_side(m)/0.5)`. Labelled `a thinking <profile> (human moves filtered by the engine)`. ≈ 0.7 s/ply.
The chosen move's `human_prob` is stored per ply.

### 9.3 Stop conditions (`stop_reason`)
`length` (plies reached); `pass` (a sampled pass is recorded as the last token and ends the replay); `decided`: 1-visit winrate outside [0.03, 0.97] **and** `|score| ≥ 15` (9x9: ≥ 10) for 3 consecutive plies; `error`/`timeout`. A start position already decided by that test → marker `not available (position decided)`.

### 9.4 Evaluation
Trajectory = per-ply 1-visit values (JSON only). `final` = unrestricted `eval_position(…, 200, default)` at the end of each panel replay (≈ 2.1 s); long runs: 500 visits at the end (≈ 5.1 s) and their ply-12 checkpoint is the panel's final. Precomputed comparisons per level: `diff_pts = sign(M) × (final_best − final_played)` printed as `the better move is 5.2 pts ahead for Black after 12 moves (B+4.0 vs W+1.2)`.

**Storage.**
```rust
pub struct ReplayPly { pub color: Color, pub mv: String, pub human_prob: Option<f32>, pub score_lead: f64, pub winrate: f64 }
pub struct Replay { pub kind: String /* the Sequence kind */, pub panel: String /* "after_played" | "after_best" */, pub level: String /* engine|student|target|opponent */,
    pub from_turn: usize, pub after_played: bool, pub first_to_move: Color, pub start_moves: Vec<String>, pub profile_used: String, pub mode: String /* typical|realistic */,
    pub plies: Vec<ReplayPly>, pub start_score: f64, pub final_score: f64, pub final_winrate: f64, pub final_visits: u64, pub stop_reason: String, pub small_board_caveat: bool }
pub struct PanelMatrix { pub cells: Vec<(String /* kind */, Option<usize> /* index into replays */, Option<String> /* reason if missing */)>, pub diffs: Vec<(String /* level */, f64 /* pts, mover view */)> }
// TeachingCandidate: replays: Vec<Replay>, panel: PanelMatrix
```
**Markdown** (six sequence lines + the two engine lines + one comparison line per human level; markers for missing cells):
`- Sequence replay_played_student (White first, from the position after move 23): F7 D4 E3 C4 G3 H2 D2 C2 B2 E1 F2 G1 → W+2.0 [12 moves; mode typical; profile rank_10k; visits 200]`
`- Sequence replay_best_student (Black first, from the position before move 23): D4 C4 E3 D2 F3 … → B+3.1 [12 moves; mode typical; profile rank_10k; visits 200]` (written out in full — the ellipsis here is only for this document)
`- Level comparison (your level rank_10k): the better move is 5.1 pts ahead for Black after 12 moves (B+3.1 vs W+2.0).`
`- Sequence long_run_played (White first, from the position after move 23): … → W+1.2 [24 moves; mode typical; profile rank_10k_8k; visits 500]`
Regex (comparison): `^- Level comparison \((your level|target level|this opponent) (\w+)\): the (better|played) move is (\d+\.\d) pts ahead for (Black|White) after (\d+) moves \(([BW]\+\d+\.\d) vs ([BW]\+\d+\.\d)\)\.$`. Reading guide: "mode typical = moves sampled from the human network's distribution for that rank (a typical player, not the best player of that rank); the engine `point_loss` remains the severity."

**Edge cases.** Student = White handled by colour alternation from `turns[prefix].to_move`. Handicap: asymmetric profile applies. Boards < 13: `small_board_caveat` in the note (`human model unverified on small boards`). Illegal moves impossible: sampled from `humanPolicy ≥ 0`.

**Cost.** Panel replay ≈ 12 × 0.1 + 2.1 ≈ 3.3 s; 4 human panels (2 when the opponent row is "same as your level") × 8 candidates ≈ 106 s (or 53 s). Long runs: 4 featured × 2 × (24 × 0.1 + 5.1) ≈ 60 s. Realistic mode multiplies the ply cost by 7 (opt-in).

---

## 10. Missed opportunities

Pure derivation (0 s) over reviews, `plan_diff`, `local_ld` and the §1 probes. **Kill/Save/BigMove at teaching positions are re-labels of teaching candidates** (`teaching_index` set); genuinely new moments come from `Punish` (any student move) and from `BigMove` at the four after-opponent-error positions probed in §1(b).

**Kinds** (student turn index `j`, review `r_j`, opponent's previous review `r_{j−1}`):
- `Punish`: `r_{j−1}.point_loss ≥ 3.0` and `r_j.point_loss ≥ max(2.0, 0.5 × r_{j−1}.point_loss)`. `points = min(r_j.point_loss, r_{j−1}.point_loss)`, `best = r_j.alternatives[0].mv`. `punish_is_local = chebyshev(best_j, opp_move_{j−1}) ≤ 3 || best_j is a liberty of the unit containing opp_move_{j−1}`. Local → note `failed to punish <opp move>`; non-local → `let the gift slip: the biggest move was elsewhere at <best>`.
- `Kill` / `Save` (positions with `plan_diff`): a `GroupDiff` with `status_best == "dead" && status_played != "dead"` for an opponent unit (`Kill`) or `status_best == "alive" && status_played != "alive"` for a student unit (`Save`), stones ≥ 2; `points = r_j.point_loss`; move from `local_ld` (`killing_move`/`saving_move`) when it confirms `Unsettled`, else `best`.
- `BigMove` (positions with a pass probe): `urgency ≥ 8.0` (19x19) / `5.0` (≤ 13x13) **and** `played_value ≤ 0.3 × urgency`. Absolute thresholds are right here because they are combined with the relative test on the played move. `points = urgency − played_value`.
Priority when several apply: Kill/Save > Punish > BigMove; the others go to `also`.

**Window.** From `j`, walk the student's turns `j+2, j+4, …` while `chebyshev(best_k, best_j) ≤ 2 && r_k.point_loss ≥ 1.5`; `open_moves = [r_j.number, …]`; `closed_at` = the first opponent move after the last open move whose best-move location leaves the area, or `None`; `taken_at` = the student move with `rank == Some(0)` at that spot, if any. Merge overlapping windows so each opportunity is reported once.

**Selection.** Skip passes. Keep decided ones but flag. Top 6 by `points`.

**Storage.**
```rust
pub enum OpportunityKind { Punish, Kill, Save, BigMove }
pub struct MissedOpportunity { pub number: usize, pub color: Color, pub kind: OpportunityKind, pub also: Vec<OpportunityKind>, pub points: f64, pub best: String, pub played: String,
    pub opponent_error: Option<(usize, String, f64)>, pub punish_is_local: Option<bool>, pub group: Option<(Color, String, usize)>, pub urgency: Option<f64>, pub played_value: Option<f64>,
    pub open_moves: Vec<usize>, pub closed_at: Option<(usize, String)>, pub taken_at: Option<usize>, pub decided: bool, pub teaching_index: Option<usize> }
// GameAnalysis: missed: Vec<MissedOpportunity>; TeachingCandidate: missed: Option<MissedOpportunity>
```
**Markdown section** `## Missed opportunities` (before Game arc facts; ≤ 6 rows):
`| Move | Kind | Points for Black | Best | Played | Open at (your moves) | Closed by | Note |`
`| 23 | kill | 7.6 | D2 | G7 | 23, 25, 27 | 28 White C2 | White C3 (3 stones) killable; teaching candidate |`
`| 44 | punish | 4.0 | Q3 | K10 | 44 | — | failed to punish White's move 43 R3 (lost 6.1) |`
Note fragments are fixed: `<C> <anchor> (<n> stones) killable|savable`, `failed to punish <C>'s move <n> <mv> (lost <x>)`, `let the gift slip: the biggest move was elsewhere at <mv>`, `move worth <x> pts, played move worth <y>`, `teaching candidate`, `game already decided`.

---

## 11. Rank estimation from human-network likelihood

**Scale.** The integer rank scale of §0.10 (`−20 … 8`). Coarse set `C = {−15, −9, −5, −1}` (15k, 9k, 5k, 1k); after the coarse argmax `v*`, evaluate `v*±2` (clamped, not yet evaluated), re-take the argmax, then `±1` around it. ≤ 8 symmetric profiles.

**Query per profile.** One query on the game move list, `analyzeTurns = S`, `maxVisits = 1`, `human_profile`, priority 5; `humanPolicy` is returned for every turn regardless of colour (verified), so one pass covers both players. `S` = turns `k` where `moves[k]` is not a pass and `0.01 ≤ turns[k].winrate ≤ 0.99`; if `|S| > 120`, subsample per colour with stride `ceil(|S_c| / 60)`, stratified by phase. Per review store `rank_probs: Vec<(String, f32)>` (≤ 11 entries).

**Likelihood with clipped log-ratios.** Per move `l_r(k) = ln max(h_r(k, played_k), 1e-4)`; reference `m(k)` = median of `l_r(k)` over the four coarse profiles (fixed after the coarse stage); `LL_c(r) = Σ_{k ∈ S_c} clip(l_r(k) − m(k), −2, +2)`. Clipping the *ratio* at ±2 nats means one freak move can shift a profile by at most 2 nats relative to the field (a probability floor alone still allows −6.9 nats against a total inter-rank gap of 3–12). Per phase over `S_c ∩ phase` when ≥ 8 moves.

**Asymmetric refinement (student only).** If `|v_B* − v_W*| ≥ 3`, re-evaluate the student's `{v*−1, v*, v*+1}` with `rank_{B}_{W}` where the opponent's colour is fixed at its symmetric estimate (3 more 1-visit passes); the student's final estimate/range come from these; the opponent keeps the symmetric one. `RankSide.asymmetric: bool`.

**Posterior.** Uniform prior over evaluated profiles: `post(r) = exp(LL(r) − LL_max) / Σ`. Estimate = argmax. Credible set = evaluated profiles with `LL ≥ LL_max − 2.0`; range = its min..max on the scale; `uncertain` if the range spans ≥ 6 (about six stones). Fewer moves automatically widen it.

**Rank-revealing moves.** `r_up = profile_of(stronger(v_est, 3))` (evaluated if missing, one more pass): `reveal_k = l_est(k) − l_up(k)`; top 5 with `reveal_k ≥ 1.5` (moves strong players almost never play); top 3 by `−reveal_k` (moves that look stronger than the estimate).

**Minimums / caveats.** ≥ 15 non-pass live moves per side, else `not available (too few moves)`. Boards < 13: `small_board_caveat`. Handicap: `handicap_caveat`. Engine opponents: estimated for information, never used as a profile. Publish a point estimate only when the range spans ≤ 4; otherwise print the range with "uncertain" (calibration §15.8 may tighten this).

**Storage.**
```rust
pub struct RankLikelihood { pub profile: String, pub value: i32, pub ll: f64, pub n_moves: usize, pub posterior: f32 }
pub struct RankPhase { pub phase: String, pub estimate: Option<String>, pub range: Option<(String, String)>, pub n_moves: usize }
pub struct RankSide { pub color: Color, pub estimate: String, pub label: String /* "about 7k" */, pub range: (String, String), pub uncertain: bool, pub asymmetric: bool,
    pub ladder: Vec<RankLikelihood>, pub phases: Vec<RankPhase>, pub sgf_rank: Option<String>, pub n_moves: usize,
    pub revealing_moves: Vec<(usize, String, f32, f32, f32)> /* (number, mv, p_est, p_up, nats) */ }
pub struct RankEstimate { pub student: Option<RankSide>, pub opponent: Option<RankSide>, pub profiles_evaluated: Vec<String>, pub small_board_caveat: bool, pub handicap_caveat: bool }
// GameAnalysis: rank_estimate: Option<RankEstimate>, profiles: Profiles
```
**Markdown section** `## Rank estimate` (≤ 12 lines):
`- Student (Black): about 7k (credible 9k–5k) from 61 moves; SGF says 8k; opponent-aware profiles used.`
`- Student by phase: opening 9k (12 moves, uncertain); middlegame 6k (34 moves); endgame 8k (15 moves).`
`- Opponent (White): about 5k (credible 7k–3k) from 58 moves; engine name, estimate for information only.`
`- Rank-revealing moves (Black): 23 G7 (7k 31%, 4k 2%); 41 B2 (7k 18%, 4k 1%).`
`- Caveats: handicap game; human model unverified on this board size.`
Regex (side): `^- (Student|Opponent) \((Black|White)\): about (\d+[kd]) \(credible (\d+[kd])–(\d+[kd])(?:, uncertain)?\) from (\d+) moves(?:; SGF says ([^;]+))?(?:; (.*))?\.$`.

**Cost.** 19x19, 250 moves: `|S| ≤ 120` → ≈ 12 s per pass × (≤ 8 + 3 + 1) ≈ 100–145 s worst case, typically 7–9 passes ≈ 90–110 s. 9x9, 66 moves: ≈ 6.7 s × 8 ≈ 55 s. Runs outside the probe budget (it feeds `profiles`) but is bounded to ≤ 12 passes.

---

## 12. Orchestration, budget planner and cost

### 12.1 Stages in `analyze_game` (each updates the job's partial JSON/live view)
1. Pass 1 (existing).
2. Derived, 0 s: reviews, teaching selection, praise (from pass-1 reviews), `difficulty`, `played_sente` (from turns), Punish opportunities, status changes.
3. Deep pass 2 (`deep_turns` + praised before-positions) with `include_moves_ownership`; final `select`; the `missing` key-positions ownership query (§4); `plan_diff`, Kill/Save opportunities; re-derive §5/§3.
4. Rank estimation (§11); `resolve_profiles`.
5. Target-profile 1-visit pass (existing; derived target if unset) and a student-profile 1-visit pass when `human_profile` was unset but estimated (so `human_policy`/`human_prob` exist).
6. **Calibration**: the first 4 pass probes (teaching candidates, featured first) are timed; fit `a`, `r` by least squares on `(visits, seconds)` together with the deep-pass timing; build the `BudgetPlan`.
7. Execute the plan in priority order (§12.2); the §1 baseline/labels, §2 values, §3 best-sente, §10 BigMove are derived as their inputs arrive.
`deadline = start_of_stage_6 + budget_seconds` is a safety net only; the plan decides what runs.

### 12.2 Budget planner (`BudgetPlan`)
Every probe batch is declared up front with an id, a kind, an estimated cost `Σ t(v)` over its positions, and a **priority class**:
- **P0** (always, unless the budget is 0): pass probes for teaching candidates; tree children for all candidates; the §8 policy query; the `missing` ownership query (stage 3, outside the budget).
- **P1**: the four human panel replays per candidate (all candidates), in candidate order (featured first); §8 evaluations for featured candidates.
- **P2**: local L&D for the first 4 units; grandchildren for featured candidates.
- **P3**: pass probes for after-opponent-error positions, endgame extras, praise; temperature curve; L&D units 5–8.
- **P4**: long runs for featured candidates.
- **P5**: `realistic` replay mode and `thinking` refutation evaluations (opt-ins).
Algorithm: after calibration, walk the batches in (class, declared order) and include each while `cumulative_estimate ≤ budget_seconds × 1.05`; everything else is skipped with `skipped_for_budget.push(format!("{} ({}, est. {:.0} s)", id, kind, est))`. Executed in included order (class by class), 2 in flight. Re-plan after each class with the measured rates: a class-P3 batch may be re-admitted if earlier classes ran faster, and a batch whose start would exceed the deadline by > 20% is skipped even if planned. Dependencies are respected inside the order: `replay_played_opponent` waits for the §8 reply of that candidate; L&D waits for `plan_diff`.
The Game-information table prints `Probe budget: 300 s; used 287 s; skipped: long runs (P4, est. 60 s), life and death for moves 45, 52 (P3, est. 12 s)` and `Probe timing: pass 16×4.0 s, tree 30×3.0 s, replay 384×0.11 s, …` from `seconds_by_kind`.

Query priorities: main passes 0, probes 5, live `explore` 10. Concurrency: `in_flight` permits (default 2), never touching `numAnalysisThreads`.

### 12.3 Cost table (wall time, sequential model `0.1 + v/100`; 2-in-flight with GPU headroom cuts stages 6–7 by up to ~40%)

| Item | 9x9, 66 moves, 8 candidates (4 featured) | 19x19, 250 moves, 8 candidates (4 featured) |
|---|---|---|
| Existing v4 pipeline (pass 1, deep pass, target pass) | ≈ 410 s | ≈ 800 s |
| Praised before-positions in the deep pass, `missing` ownership query | 30 + 15 s | 30 + 30 s |
| §11 rank ladder (≤ 12 one-visit passes, typ. 8–9) | 55 s | 100 s |
| Student-profile pass when estimated | 7 s | 25 s |
| §1/§2 pass probes (≤ 16 × 4.1) + curve | 66 + 8 s | 66 + 28 s |
| §3, §4, §5, §10 derivations | 0 s | 0 s |
| §7 trees (4 × 19 + 4 × 9.3) | 113 s | 113 s |
| §8 refutations (1 + ≤ 4 × 3.1) | 14 s | 14 s |
| §6 L&D (≤ 8 units × 2 × 3.1) | 50 s | 50 s |
| §9 panels (8 × 4 × 3.3) + long runs (4 × 2 × 7.5) | 106 + 60 s | 106 + 60 s |
| **Probe stages unbudgeted** | ≈ 420 s | ≈ 440 s |
| **Probe stages with the 300 s budget** | 300 s (drops P4, part of P3) | 300 s (drops P4, part of P3) |
| **Grand total, budgeted** | ≈ 13.5 min | ≈ 21.5 min |
These are lower bounds when KaTrain's engine shares the GPU; `elapsed_seconds` and `probe_stats` are reported so the user can tune `budget_seconds`, `featured`, `in_flight`.

---

## 13. Report changes summary (format 5)
- `Report format: 5`. Reading guide gains one bullet per new label (urgent/normal/quiet relative to the stage; only move/sharp/normal/wide; sente/gote/punished locally; alive/dead/unsettled/unclear), the perspective rule, the sequence-line grammar, the "missing = unknown" rule, the miai/deiri note, the restricted-search warning, and the replay-mode note.
- `## Teaching candidates`: the `Evidence available` line; table gains `Featured`, `Urgency (for <colour>)`, `Difficulty` columns; each candidate block = existing hints + position + diagram + candidate table + the new lines in this fixed order: Urgency, Move values (≤ 4), Sente/gote (played, best), Shared biggest point, Plan difference (2 lines), Difficulty (2 lines), Local life and death (per unit ≤ 5 lines), Tree children (≤ 4 × 2 lines), Realistic refutation (3 lines), panel sequences (8 lines) + level comparisons (≤ 3) + long runs (2), Missed opportunity (1), Theme/Phase. Missing items are markers.
- `### Good moves worth praising`: `Urgency`, `Difficulty` columns.
- New `## Missed opportunities` (≤ 6 rows) and `## Rank estimate` (≤ 12 lines) before `## Game arc facts`.
- Game-flow table: `Move value` column. Game-information table: probe budget and timing rows, `Profiles: student rank_10k (SGF rank), target rank_7k (derived), opponent rank_8k (rank in the bot's name)`.
- Move-by-move: teaching candidates and praised moves as one-liners pointing to their blocks; opponent full entries ≤ 3.

## 14. Edge cases (cross-cutting)
- **Passes in the record.** `to_move` from `rootInfo`. §1 and §6 never create two consecutive passes. §11 skips pass moves. §3: reply `pass` → Gote. §9: a sampled pass ends the replay as the last token. PVs are cut at a pass.
- **Decided games.** All new quantities are score-based; winrate only for the decided flag and replay stop. Replays from decided starts → marker. Difficulty/urgency/plan diffs still computed with `decided_before`.
- **Handicap.** Setup stones via `initialStones`; `who_moves_first` = White; asymmetric `pair`; `handicap_caveat`; komi from the SGF.
- **Board sizes.** `R` (§3) 2/3/4; BigMove absolute threshold 5/8; decided score 10/15; L&D regions may cover most of a 9x9 board; rectangular boards via `to_gtp(size_y)`; human model on < 13x13 → `small_board_caveat` in every human line's note.
- **Illegal moves / ko.** Only `pass`, `moveInfos` moves or policy entries ≥ 0 are appended; any `error` → warning + marker; the job continues.
- **Student = White.** `sign(M)`; `pair` Black-first; replays alternate from `to_move`; `RankEstimate.student.color = White`.
- **Missing human model.** Stages 4–5, §8, §9 human cells skipped with `no human model` markers; §5 human line marker; §7 without human-labelled nodes; `Profiles` all `None`; the engine row of the panel matrix is always filled.
- **Engine opponent.** Rank in the bot name → that profile; otherwise the student's profile with the label; never a KataGo side. §8 still runs (labelled). §11 estimates the bot's apparent rank for information.
- **Very short games** (< 15 student moves): `Rank estimate: not available (too few moves)`.
- **Resume.** Probe results are not resumable; a resumed job re-runs stages 4–7 within the budget.
- **Cancellation.** Every probe awaits `cancel`; the planner checks `cancel.is_cancelled()` before each batch; `run_probe` always unregisters.
- **JSON size.** Candidate ownership only on teaching before-positions (≤ 4 arrays, 2 decimals); `human_policy_opponent` on ≤ 8 turns; `rank_probs` ≤ 11 floats per review; replay trajectories ≤ 24 plies × 8 replays per candidate.

## 15. Empirical verification checklist (in this order; results freeze the thresholds)
1. **Throughput first**: time 10 sequential 300-visit probes vs 10 with 2 in flight while a 1000-visit deep pass runs; set `in_flight`, the initial `a`, `r`, and the default `budget_seconds`.
2. `utility` sign under `reportAnalysisWinratesAs=BLACK`: a clearly White-winning position with White to move must give negative `rootInfo.utility` if Black-view; fix `u_O = sign(O) × utility` accordingly (§8/§9 realistic mode).
3. Pass after a pass under both `japanese` and `chinese`: send `[…,[W,pass],[B,pass]]`, analyse the last turn, record the response shape (normal result / encore / `noResults` / error). The skip rule stays regardless.
4. `humanPolicy` length `sx*sy + 1` with −1 for illegal points; asymmetric dan/kyu mixes (`rank_3k_2d`) accepted; **cache check**: the same position queried with `rank_20k` then `rank_5d` back-to-back must return different `humanPolicy` (max abs diff > 0.05), otherwise the ladder would be flat.
5. `allowMoves` `untilDepth` counts plies from the root (12 → six moves each) and `pass` in the list is honoured, including with an inserted leading pass.
6. L&D fixtures (expected verdicts): dead L-group in the corner → `Dead`; six stones on the second line **in the corner** → `Alive` (四死六活); six stones on the second line **mid-side** → `Unsettled` (二線六死八活: dies attacked first, lives moving first); eight mid-side → `Alive`; bent four in the corner → `Dead`; straight three → `Unsettled` with the middle point as both killing and saving move; a seki → `Unclear` with the ambiguity note.
7. Value identities on 20 teaching positions: `urgency − played_value = point_loss` (exact), `|value(best) − urgency| ≤ 1.5`, and `|Σ delta − point_loss| ≤ max(3, 0.5 × point_loss)` for plan diffs; record the observed Σ ownership/score ratio under Japanese rules.
8. Rank ladder calibration on ≥ 10 SGFs with known OGS/KGS ranks (19x19) and a few 9x9 games: check the estimate is within ±3 stones and that the credible range covers the true rank ≥ 80% of the time; decide whether 9x9 estimates are published or only the caveat; check that adjacent-rank per-move log-ratios are informative enough (mean |Δ| per move).
9. Replay realism: for 5 known-rank games compare the mean engine loss per move of `typical` replays with the players' own mean loss in the same phase; they should be within ±1 pt (argmax replays will be noticeably lower — evidence for the default).
10. `includeMovesOwnership` on a 19x19 deep pass (~40 positions): no measurable slowdown, JSON size acceptable.
11. Sente rule sanity on 20 positions: no move with `loss_vs_best ≥ 1.5` is labelled sente; hand-check 10 `Sente` labels against Go judgement (the answer is a defence, the mover keeps the initiative).
12. Fake engine (`tests/fake_katago.py`): return `humanPolicy` depending on the profile string (deterministic argmax that shifts with rank), `utility` per candidate, per-candidate `ownership` when requested; pipeline test asserts: `urgency` on every teaching candidate, a `VariationTree` of depth 2 on featured ones, a `LocalLifeDeath` entry, eight panel cells filled or marked per candidate, a `Replay` with `stop_reason`, a `RankEstimate` with a range, `missed` non-empty, Markdown containing `## Missed opportunities`, `## Rank estimate`, the `Evidence available:` line, ≥ 6 `- Sequence ` lines per candidate, no bare signed parentheticals (§0.5 regex), the word count under §0.8, and `parse_review.py` round-tripping every §0.9 field.

## 16. Decisions on the former open questions
1. **Replay move choice**: `typical` seeded sampling by default; `realistic` (utility-weighted) opt-in and labelled; argmax is never used for replays. Refutation reply = most common (argmax) because it is a single explainable fact; the thinking variant is JSON/opt-in.
2. **Level replays for every candidate**: yes — four human panel cells per teaching candidate at a uniform 12 plies (~106 s for 8 candidates, P1), so any lesson the skill picks has a working toggle; long runs stay featured-only.
3. **Featured count**: 4 (expensive items only: grandchildren, L&D, refutation evaluation, long runs). The skill is told to prefer featured candidates.
4. **Engine opponents**: never modelled with a KataGo side; rank from the bot name, else the student's profile with an explicit label; the `opponent` row is then "same as your level".
5. **`unpunished_points`**: kept, printed as a fact ("leaves x pts of the punishment unplayed"), never as a grade; `category` stays engine-based.
6. **L&D puzzles**: allowed via `from_your_game: true` with the printed region and a validator exception; L&D lines are also lesson-panel material.
7. **Rank publication**: point estimate only when the credible range ≤ 4 stones; always with the range; 9x9 decided by §15.8.
8. **Budget**: planner with priority classes (§12.2), not a deadline cut; default 300 s from calibration; rank ladder outside the budget but capped.
9. **Sente for mistakes**: never; `PunishedLocally` or `Gote`.
10. **Seki**: not detected from ownership; `Unclear` with a note.
Nothing remains for the user unless §15.8 shows the human model cannot separate adjacent ranks on 9x9, in which case 9x9 rank estimates are replaced by the caveat line.